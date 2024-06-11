pub trait HardwareFlashDevice {
    type Error;

    /// Reads flash contents into `buf`, starting at `addr`.
    fn read(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Self::Error>;

    /// The Sector Erase instruction sets all memory within a specified sector
    /// to the erased state of all 1s (FFh).
    fn sector_erase(&mut self, addr: u32) -> Result<(), Self::Error>;

    /// The Page Program instruction allows from one byte to 256 bytes (a page) of data
    /// to be programmed at previously erased (FFh) memory locations.
    fn page_program(&mut self, addr: u32, data: &[u8]) -> Result<(), Self::Error>;

    /// Chip Erase (see datasheet 8.2.18)
    /// The Chip Erase instruction sets all memory within the device to the erased
    /// state of all 1s (FFh).
    fn chip_erase(&mut self) -> Result<(), Self::Error>;
}
