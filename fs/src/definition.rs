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
    pub block_bitmap: [u8; 128],    // 128 * 8 = 1024 = 1 block

    /// The bitmap which indicates whether each inode part is used.
    pub inode_block_bitmap: [u8; 1024 * 1024 / core::mem::size_of::<Inode>()],  // 1024 * 1024 bytes = 1024 blocks, divide by inode size = total inode number

    /// The bitmap which indicates whether each inode is used.
    pub inode_bitmap: [u8; 128],    // 128 * 8 = 1024 = 1 block
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
            block_bitmap: [0; 128],
            inode_block_bitmap: [0; 1024 * 1024 / core::mem::size_of::<Inode>()],
            inode_bitmap: [0; 128],
        }
    }
}

/// The definition of the file type
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    /// The regular file.
    Regular = 0,

    /// The directory.
    Directory = 1,

    /// The device file.
    Device = 2,
}

/// The definition of the inode.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Inode {
    /// The ID of this inode.
    pub inode_id: u32,

    /// The file type.
    /// 
    /// # Number of this parameter
    /// 0: regular file;
    /// 
    /// 1: directory;
    pub file_type: FileType,

    /// The head block of the file.
    pub head_block: u32,

    /// The file length in bytes.
    pub file_length: u64,
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

/// The entry point of directory.
pub struct DirEntry {
    /// The inode number of the directory.
    pub inode: u32,

    /// The name of the directory, which contains up to 255 characters.
    pub name: [u8; 256],
}

impl DirEntry {
    pub const fn empty() -> Self {
        Self {
            inode: 0,
            name: [0; 256],
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, core::mem::size_of::<Self>())
        }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self as *mut Self as *mut u8, core::mem::size_of::<Self>())
        }
    }
}