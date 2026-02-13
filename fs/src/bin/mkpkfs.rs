//! The tool to create the proka file system.
use proka_fs::{FileSystem, BlockDevice};
use std::io::{Seek, SeekFrom, Read, Write};
use std::fs::File;

const BLOCK_SIZE: usize = 1024;

// Implement the block device for the file.
pub struct FileBlockDevice(File);

impl BlockDevice for FileBlockDevice {
    fn read_block(&mut self, block_num: u32, offset: u32, buf: &mut [u8]) -> Result<(), &'static str> {
        self.0.seek(SeekFrom::Start(block_num as u64 * BLOCK_SIZE as u64 + offset as u64))
            .map_err(|_| "Failed to seek to block")?;
        self.0.read_exact(buf).map_err(|_| "Failed to read block")
    }

    fn write_block(&mut self, block_num: u32, offset: u32, buf: &[u8]) -> Result<(), &'static str> {
        self.0.seek(SeekFrom::Start(block_num as u64 * BLOCK_SIZE as u64 + offset as u64))
            .map_err(|_| "Failed to seek to block")?;
        self.0.write_all(buf).map_err(|_| "Failed to write block")
    }
}

fn main() {
    println!("Hello, world!");
    // TODO: Implement the tool to create the proka file system.
}

