# Winbond W25 NOR flash driver

This is a platform agnostic Rust driver for the Winbond W25X and W25Q SpiFlash
SPI devices, using the [`embedded-hal`] (v1) traits.

This driver allows you to:

- Read flash contents into a buffer (see `read()`)
- Erase sectors (see `sector_erase()`)
- Program pages (see `page_program()`)
- Chip erase (see `chip_erase()`)
- Software reset (see `software_reset()`)
- Read the manufacturer and device ID (see `read_manufacturer_id()`)
- Read the JEDEC ID (see `read_jedec_id()`)
- Set the write enable latch (see `write_enable()`)

It supports:

- Blocking SPI using `embedded-hal 1.0`

## The devices

More information about the Winbond W25X and W25Q devices can be found in the
[winbond_w25x](https://www.winbond.com/hq/product/code-storage-flash-memory/serial-nor-flash/)
information page.

An example datasheet can be viewed for the
[W25Q128JV](https://datasheet.lcsc.com/lcsc/1912111437_Winbond-Elec-W25Q128JVSIQ_C113767.pdf)

## Usage

To use this driver, import this crate and an `embedded-hal` (v1) implementation.

The following example shows use of this driver using the
[embassy](https://embassy.dev/) framework:

```rust
let config = embassy_stm32::Config::default();
let p = embassy_stm32::init(config);
let spi_config = embassy_stm32::spi::Config::default();

let cs_pin = embassy_stm32::gpio::Output::new(AnyPin::from(p.PB9), Level::High, Speed::VeryHigh);
let spi = embassy_stm32::spi::Spi::new(
    p.SPI2, p.PF9, p.PB15, p.PA10, p.DMA1_CH5, p.DMA1_CH4, spi_config,
);

// Combine the SPI bus and the CS pin into a SPI device. This now implements SpiDevice!
let spi_device = embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs_pin, embassy_time::Delay).unwrap();
let flash_spi = winbond_w25_nor_flash_rs::comms::FlashSpi::init(spi_device).unwrap();

// Do stuff
flash_spi.chip_erase().unwrap();
flash_spi.page_program(0, &[0x01, 0x02, 0x03]).unwrap();
let mut data = [0u8; 3];
flash_spi.read(0x00, &mut data).unwrap();
```
