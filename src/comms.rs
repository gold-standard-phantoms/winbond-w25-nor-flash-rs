/// Refer to datasheet:
/// https://datasheet.lcsc.com/lcsc/1912111437_Winbond-Elec-W25Q128JVSIQ_C113767.pdf
use crate::error::Error;
use crate::identification::Identification;
use core::fmt::Debug;
use embedded_hal::spi::{Operation, SpiDevice};
use hardware_traits::HardwareFlashDevice;

// #[derive(Debug)]
pub struct FlashSpi<SPI> {
    spi: SPI,
}
impl<SPI> Debug for FlashSpi<SPI> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "FlashSPI")
    }
}

enum Opcode {
    /// Read the 8-bit manufacturer and device IDs.
    ReadMfDId = 0x90,
    /// Read 16-bit manufacturer ID and 8-bit device ID.
    ReadJedecId = 0x9F,
    /// Set the write enable latch.
    WriteEnable = 0x06,
    /// Read the 8-bit status register.
    ReadStatus = 0x05,
    Read = 0x03,
    PageProg = 0x02,
    SectorErase = 0x20,
    ChipErase = 0xC7,
    EnableReset = 0x66,
    Reset = 0x99,
}

defmt::bitflags! {
    /// Status register bits.
    pub struct Status: u8 {
        /// Erase or write in progress.
        const BUSY = 1 << 0;
        /// Status of the **W**rite **E**nable **L**atch.
        const WEL = 1 << 1;
        /// The 3 protection region bits.
        const PROT = 0b00011100;
        /// **S**tatus **R**egister **W**rite **D**isable bit.
        const SRWD = 1 << 7;
    }
}

impl<SPI> HardwareFlashDevice for FlashSpi<SPI>
where
    SPI: SpiDevice,
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
    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Error<SPI>> {
        // TODO what happens if `buf` is empty?

        self.wait_done()?;
        let spi_result = self.spi.transaction(&mut [
            Operation::Write(&[
                Opcode::Read as u8,
                (addr >> 16) as u8,
                (addr >> 8) as u8,
                addr as u8,
            ]),
            Operation::Read(buf),
        ]);
        spi_result.map(|_| ()).map_err(Error::Spi)
    }
    /// Sector erase (see datasheet 8.2.15)
    /// The Sector Erase instruction sets all memory within a specified sector
    /// (4K-bytes) to the erased state of all 1s (FFh). A Write Enable instruction
    /// must be executed before the device will accept the Sector Erase Instruction
    /// (Status Register bit WEL must equal 1). The instruction is initiated by
    /// driving the /CS pin low and shifting the instruction code “20h” followed
    /// a 24-bit sector address (A23-A0)
    fn sector_erase(&mut self, addr: u32) -> Result<(), Error<SPI>> {
        // Address should be the start of a sector
        self.wait_done()?;
        self.write_enable()?;

        let cmd_buf = [
            Opcode::SectorErase as u8,
            (addr >> 16) as u8,
            (addr >> 8) as u8,
            addr as u8,
        ];
        self.command(&cmd_buf)?;

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
    fn page_program(&mut self, addr: u32, data: &[u8]) -> Result<(), Error<SPI>> {
        self.wait_done()?;
        self.write_enable()?;
        if !self.is_wel()? {
            defmt::warn!("WEL should be set: {:?}", self.read_status()?);
        }

        let spi_result = self.spi.transaction(&mut [
            Operation::Write(&[
                Opcode::PageProg as u8,
                (addr >> 16) as u8,
                (addr >> 8) as u8,
                addr as u8,
            ]),
            Operation::Write(data),
        ]);
        spi_result.map(|_| ()).map_err(Error::Spi)?;
        Ok(())
    }
    /// Chip Erase (see datasheet 8.2.18)
    /// The Chip Erase instruction sets all memory within the device to the erased
    /// state of all 1s (FFh). A Write Enable instruction must be executed before
    /// the device will accept the Chip Erase Instruction (Status Register bit WEL
    /// must equal 1). The instruction is initiated by driving the /CS pin low and
    /// shifting the instruction code “C7h” or “60h”.
    fn chip_erase(&mut self) -> Result<(), Error<SPI>> {
        self.wait_done()?;
        self.write_enable()?;
        let cmd_buf = [Opcode::ChipErase as u8];
        self.command(&cmd_buf)?;
        Ok(())
    }
}

impl<SPI> FlashSpi<SPI>
where
    SPI: SpiDevice,
{
    /// Software reset (see datasheet 6.4)
    /// The W25Q128JV can be reset to the initial power-on state by a software Reset
    /// sequence. This sequence must include two consecutive instructions: Enable Reset
    /// (66h) & Reset (99h). If the instruction sequence is successfully accepted, the
    /// device will take approximately 30μS (tRST) to reset. No instruction will be
    /// accepted during the reset period
    pub fn software_reset(&mut self) -> Result<(), Error<SPI>> {
        self.wait_done()?;
        self.write_enable()?;
        let cmd_buf = [Opcode::EnableReset as u8];
        self.command(&cmd_buf)?;
        self.wait_done()?;
        let cmd_buf = [Opcode::Reset as u8];
        self.command(&cmd_buf)?;
        Ok(())
    }
}

impl<SPI> FlashSpi<SPI>
where
    SPI: SpiDevice,
{
    pub fn is_busy(&mut self) -> Result<bool, Error<SPI>> {
        let status = self.read_status()?;
        Ok(!(status & Status::BUSY).is_empty())
    }
    pub fn is_wel(&mut self) -> Result<bool, Error<SPI>> {
        let status = self.read_status()?;
        Ok(!(status & Status::WEL).is_empty())
    }

    pub fn init(spi: SPI) -> Result<Self, Error<SPI>> {
        let mut this = Self { spi };
        let status = loop {
            let status = this.read_status()?;
            if (status & (Status::BUSY | Status::WEL)).is_empty() {
                break status;
            }
            defmt::warn!("Flash is not ready: {:?}", status);
        };
        defmt::debug!("Initial status: {:?}", status);
        Ok(this)
    }

    /// Writes a command to the SPI bus
    fn command(&mut self, bytes: &[u8]) -> Result<(), Error<SPI>> {
        self.spi
            .transaction(&mut [Operation::Write(bytes)])
            .map_err(Error::Spi)?;
        Ok(())
    }

    /// Writes a command to the SPI bus and replaces the bytes inplace with the read bytes
    fn command_with_response(
        &mut self,
        instruction: &[u8],
        response: &mut [u8],
    ) -> Result<(), Error<SPI>> {
        self.spi
            .transaction(&mut [Operation::Write(instruction), Operation::Read(response)])
            .map_err(Error::Spi)?;
        Ok(())
    }

    /// Reads the status register.
    pub fn read_status(&mut self) -> Result<Status, Error<SPI>> {
        let mut response = [0u8; 2];
        self.command_with_response(&[Opcode::ReadStatus as u8], &mut response)?;

        Ok(Status::from_bits_truncate(response[1]))
    }

    pub fn read_manufacturer_device_id(&mut self) -> Result<[u8; 2], Error<SPI>> {
        let mut response = [0u8; 2];
        self.command_with_response(&[Opcode::ReadMfDId as u8, 0, 0, 0], &mut response)?;
        Ok(response)
    }

    /// Reads the JEDEC manufacturer/device identification.
    pub fn read_jedec_id(&mut self) -> Result<Identification, Error<SPI>> {
        // Optimistically read 12 bytes, even though some identifiers will be shorter
        let mut buf: [u8; 12] = [0; 12];
        buf[0] = Opcode::ReadJedecId as u8;
        self.command_with_response(&[Opcode::ReadJedecId as u8], &mut buf)?;

        // Skip buf[0] (SPI read response byte)
        Ok(Identification::from_jedec_id(&buf[1..]))
    }

    /// Block until the status of the device is not busy
    fn wait_done(&mut self) -> Result<(), Error<SPI>> {
        while self.read_status()?.contains(Status::BUSY) {}
        Ok(())
    }

    /// From datasheet section 8.2.1
    /// The Write Enable instruction sets the Write Enable Latch (WEL) bit
    /// in the Status Register to a 1. The WEL bit must be set prior to every Page Program,
    /// Quad Page Program, Sector Erase, Block Erase, Chip Erase, Write Status Register
    /// and Erase/Program Security Registers instruction. The Write Enable instruction is
    /// entered by driving /CS low, shifting the instruction code “06h” into the Data
    /// Input (DI) pin on the rising edge of CLK, and then driving /CS high.
    fn write_enable(&mut self) -> Result<(), Error<SPI>> {
        let cmd_buf = [Opcode::WriteEnable as u8];
        self.command(&cmd_buf)?;
        Ok(())
    }
}
