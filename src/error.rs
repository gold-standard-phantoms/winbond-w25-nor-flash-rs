use core::fmt::{self, Debug};
use defmt::{Format, Formatter};
use embedded_hal::spi::SpiDevice;

/// The error type used by this library.
///
/// This can encapsulate an SPI or GPIO error, and adds its own protocol errors
/// on top of that.
pub enum Error<SPI: SpiDevice> {
    /// An SPI transfer failed.
    Spi(SPI::Error),
}

impl<SPI: SpiDevice> Format for Error<SPI>
where
    SPI::Error: Debug,
{
    fn format(&self, fmt: Formatter) {
        match self {
            Error::Spi(_spi) => defmt::write!(fmt, "Error::Spi"),
        }
    }
}
impl<SPI: SpiDevice> Debug for Error<SPI>
where
    SPI::Error: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Spi(spi) => write!(f, "Error::Spi({:?})", spi),
        }
    }
}
