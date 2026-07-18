pub mod fat32;
pub mod vfs;
pub mod file;
pub mod offset;

pub trait BlockDevice {
    fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str>;
    fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<(), &'static str>;
    fn block_size(&self) -> u32;
}