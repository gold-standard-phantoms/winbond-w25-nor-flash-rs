use core::fmt::Debug;

use crate::comms::{Opcode, Status};
/// Refer to datasheet:
/// https://datasheet.lcsc.com/lcsc/1912111437_Winbond-Elec-W25Q128JVSIQ_C113767.pdf
use crate::error::Error;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::{Operation, SpiDevice};
use hardware_traits::AsyncHardwareFlashDevice;

pub struct AsyncFlashSpi<SPI, D> {
    pub spi: SPI,
    delay: D,
}

impl<SPI, D> Debug for AsyncFlashSpi<SPI, D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AsyncFlashSpi").finish()
    }
}

impl<SPI, D> AsyncHardwareFlashDevice for AsyncFlashSpi<SPI, D>
where
    SPI: SpiDevice,
    D: DelayNs,
{
    type Error = Error<SPI>;
    /// From datasheet section 8.2.6 (Read Data (03h))
    /// Reads flash contents into `buf`, starting at `addr`.
    ///
    /// Note that `addr` is not fully decoded: Flash chips will typically only
    /// look at the lowest `N` bits needed to encode their size, which means
    /// that the contents are "mirrored" to addresses that are a multiple of the
    /// flash size. Only 24 bits of `addr` are transferred to the device in any
    /// case, limiting the maximum size of 25-series SPI flash chips to 16 MiB.
    ///
    /// # Parameters
    ///
    /// * `addr`: 24-bit address to start reading at.
    /// * `buf`: Destination buffer to fill.
    async fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Error<SPI>> {
        // TODO what happens if `buf` is empty?

        self.wait_done().await?;
        let spi_result = self
            .spi
            .transaction(&mut [
                Operation::Write(&[
                    Opcode::Read as u8,
                    (addr >> 16) as u8,
                    (addr >> 8) as u8,
                    addr as u8,
                ]),
                Operation::Read(buf),
            ])
            .await;
        spi_result.map(|_| ()).map_err(Error::Spi)
    }

    /// Sector erase (see datasheet 8.2.15)
    /// The Sector Erase instruction sets all memory within a specified sector
    /// (4K-bytes) to the erased state of all 1s (FFh). A Write Enable instruction
    /// must be executed before the device will accept the Sector Erase Instruction
    /// (Status Register bit WEL must equal 1). The instruction is initiated by
    /// driving the /CS pin low and shifting the instruction code “20h” followed
    /// a 24-bit sector address (A23-A0)
    async fn sector_erase(&mut self, addr: u32) -> Result<(), Error<SPI>> {
        // Address should be the start of a sector
        self.wait_done().await?;
        self.write_enable().await?;

        let cmd_buf = [
            Opcode::SectorErase as u8,
            (addr >> 16) as u8,
            (addr >> 8) as u8,
            addr as u8,
        ];
        self.command(&cmd_buf).await?;

        Ok(())
    }

    /// From datasheet section 8.2.13
    /// The Page Program instruction allows from one byte to 256 bytes (a page) of data
    /// to be programmed at previously erased (FFh) memory locations. A Write Enable
    /// instruction must be executed before the device will accept the Page Program
    /// Instruction (Status Register bit WEL= 1). The instruction is initiated by driving
    /// the /CS pin low then shifting the instruction code “02h” followed by a 24-bit
    /// address (A23-A0) and at least one data byte, into the DI pin. The /CS pin must
    /// be held low for the entire length of the instruction while data is being sent
    /// to the device
    async fn page_program(&mut self, addr: u32, data: &[u8]) -> Result<(), Error<SPI>> {
        self.wait_done().await?;
        self.write_enable().await?;
        if !self.is_wel().await? {
            defmt::warn!("WEL should be set: {:?}", self.read_status().await?);
        }

        let spi_result = self
            .spi
            .transaction(&mut [
                Operation::Write(&[
                    Opcode::PageProg as u8,
                    (addr >> 16) as u8,
                    (addr >> 8) as u8,
                    addr as u8,
                ]),
                Operation::Write(data),
            ])
            .await;
        spi_result.map(|_| ()).map_err(Error::Spi)?;
        Ok(())
    }
    /// Chip Erase (see datasheet 8.2.18)
    /// The Chip Erase instruction sets all memory within the device to the erased
    /// state of all 1s (FFh). A Write Enable instruction must be executed before
    /// the device will accept the Chip Erase Instruction (Status Register bit WEL
    /// must equal 1). The instruction is initiated by driving the /CS pin low and
    /// shifting the instruction code “C7h” or “60h”.
    async fn chip_erase(&mut self) -> Result<(), Error<SPI>> {
        while self.is_busy().await? {
            self.delay.delay_ms(100).await;
        }
        self.write_enable().await?;
        let cmd_buf = [Opcode::ChipErase as u8];
        self.command(&cmd_buf).await?;
        while self.is_busy().await? {
            self.delay.delay_ms(100).await;
        }
        Ok(())
    }
}

impl<SPI, D> AsyncFlashSpi<SPI, D>
where
    SPI: SpiDevice,
    D: DelayNs,
{
    pub async fn init(spi: SPI, delay: D) -> Result<Self, Error<SPI>> {
        let mut this = Self { spi, delay };
        let status = loop {
            let status = this.read_status().await?;
            if (status & (Status::BUSY)).is_empty() {
                break status;
            }
            defmt::warn!("Flash is not ready: {:?}. Waiting for 10ms...", status);
            this.delay.delay_ms(10).await;
        };
        defmt::debug!("Initial status: {:?}", status);
        Ok(this)
    }

    pub async fn is_busy(&mut self) -> Result<bool, Error<SPI>> {
        let status = self.read_status().await?;
        Ok(!(status & Status::BUSY).is_empty())
    }

    pub async fn is_wel(&mut self) -> Result<bool, Error<SPI>> {
        let status = self.read_status().await?;
        Ok(!(status & Status::WEL).is_empty())
    }

    /// Writes a command to the SPI bus
    async fn command(&mut self, bytes: &[u8]) -> Result<(), Error<SPI>> {
        self.spi
            .transaction(&mut [Operation::Write(bytes)])
            .await
            .map_err(Error::Spi)?;
        Ok(())
    }

    /// Writes a command to the SPI bus and replaces the bytes inplace with the read bytes
    async fn command_with_response(
        &mut self,
        instruction: &[u8],
        response: &mut [u8],
    ) -> Result<(), Error<SPI>> {
        self.spi
            .transaction(&mut [Operation::Write(instruction), Operation::Read(response)])
            .await
            .map_err(Error::Spi)?;
        Ok(())
    }

    /// Reads the status register.
    pub async fn read_status(&mut self) -> Result<Status, Error<SPI>> {
        let mut response = [0u8; 2];
        self.command_with_response(&[Opcode::ReadStatus as u8], &mut response)
            .await?;

        Ok(Status::from_bits_truncate(response[1]))
    }
    /// Block until the status of the device is not busy
    async fn wait_done(&mut self) -> Result<(), Error<SPI>> {
        while self.read_status().await?.contains(Status::BUSY) {}
        Ok(())
    }

    /// From datasheet section 8.2.1
    /// The Write Enable instruction sets the Write Enable Latch (WEL) bit
    /// in the Status Register to a 1. The WEL bit must be set prior to every Page Program,
    /// Quad Page Program, Sector Erase, Block Erase, Chip Erase, Write Status Register
    /// and Erase/Program Security Registers instruction. The Write Enable instruction is
    /// entered by driving /CS low, shifting the instruction code “06h” into the Data
    /// Input (DI) pin on the rising edge of CLK, and then driving /CS high.
    async fn write_enable(&mut self) -> Result<(), Error<SPI>> {
        let cmd_buf = [Opcode::WriteEnable as u8];
        self.command(&cmd_buf).await?;
        Ok(())
    }
}
