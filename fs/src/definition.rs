/// The definition of the super block.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct SuperBlock {
    /// The magic number to identify the file system.
    pub magic: u32,

    /// The size of each block in bytes.
    pub block_size: u32,

    /// The total number of blocks in the file system.
    pub total_blocks: u32,

    /// The block number where the data starts.
    pub data_start_block: u32,

    /// The bitmap which indicates whether each block is used.
    pub block_bitmap: [u8; 512],    // 512 * 8 = 4096 = 1 block

    /// The bitmap which indicates whether each inode is used.
    pub inode_bitmap: [u8; 512],    // 512 * 8 = 4096 = 1 block
}

impl SuperBlock {
    /// Get the super block as a byte slice.
    /// 
    /// # Returns
    /// 
    /// * `&[u8]` - The super block as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, core::mem::size_of::<Self>())
        }
    }

    /// Get the super block as a mutable byte slice.
    /// 
    /// # Returns
    /// 
    /// * `&mut [u8]` - The super block as a mutable byte slice.
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self as *mut Self as *mut u8, core::mem::size_of::<Self>())
        }
    }
}

impl Default for SuperBlock {
    fn default() -> Self {
        Self {
            magic: 0x504B4653,
            block_size: 1024,
            total_blocks: 1024,
            data_start_block: 1,
            block_bitmap: [0; 512],
            inode_bitmap: [0; 512],
        }
    }
}

/// The definition of the inode.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Inode {
    /// The file type.
    /// 
    /// # Number of this parameter
    /// 0: regular file;
    /// 
    /// 1: directory;
    pub file_type: u8,

    /// The permission of the file.
    pub permission: u8,

    /// The size of the file in bytes.
    pub size: u64,

    /// The blocks number which the file occupies.
    pub block_count: u64,

    /// The data of the file, which points to the blocks in the data area.
    pub data: [u32; 12],
}

impl Inode {
    /// Get the inode as a byte slice.
    /// 
    /// # Returns
    /// 
    /// * `&[u8]` - The inode as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, core::mem::size_of::<Inode>())
        }
    }

    /// Get the inode as a mutable byte slice.
    /// 
    /// # Returns
    /// 
    /// * `&mut [u8]` - The inode as a mutable byte slice.
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self as *mut Self as *mut u8, core::mem::size_of::<Inode>())
        }
    }

    /// Locate the inode in the file system.
    /// 
    /// # Parameters
    /// 
    /// * `inode_id` - The id of the inode.
    /// * `super_block` - The super block of the file system.
    /// 
    /// # Returns
    /// 
    /// * `(u64, usize)` - The block index and the offset of the inode in the block.
    pub fn locate(inode_id: u32, super_block: &SuperBlock) -> (u64, usize) {
        const INODE_SIZE: usize = core::mem::size_of::<Inode>();
        let inodes_per_block = super_block.block_size as usize / INODE_SIZE;
        let inode_start_block = 1u64;   // The first block is the super block, which has been used.
        let block_idx = inode_start_block + (inode_id as u64 / inodes_per_block as u64);
        let offset = (inode_id as usize % inodes_per_block) * INODE_SIZE;
        (block_idx, offset)
    }
}