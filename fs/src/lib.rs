#![no_std]

pub mod definition;
pub mod bitmap;

/// The block device driver.
pub trait BlockDevice {
    /// Read a block from the block device.
    /// 
    /// # Parameters
    /// 
    /// * `block_num` - The block number to read.
    /// * `buf` - The buffer to store the data.
    fn read_block(&self, block_num: u32, buf: &mut [u8]);

    /// Write a block to the block device.
    /// 
    /// # Parameters
    /// 
    /// * `block_num` - The block number to write.
    /// * `buf` - The data to write.
    fn write_block(&self, block_num: u32, buf: &[u8]);
}

/// The basic structure of the whole file system.
#[repr(C)]
pub struct FileSystem<B: BlockDevice> {
    /// The block device driver.
    pub block_device: B,

    /// The super block of the file system.
    pub super_block: definition::SuperBlock,
}

impl<B: BlockDevice> FileSystem<B> {
    /// Mount the file system.
    /// 
    /// # Parameters
    /// 
    /// * `bd` - The block device driver.
    /// 
    /// # Returns
    /// 
    /// * `Self` - The mounted file system.
    pub fn mount(bd: B) -> Self {
        let mut super_block = definition::SuperBlock::default();
        bd.read_block(0, &mut super_block.as_mut_bytes());
        Self {
            block_device: bd,
            super_block: super_block,
        }
    }

    /// Synchronize the file system to the block device.
    pub fn sync(&self) {
        self.block_device.write_block(0, &self.super_block.as_bytes());
    }
}