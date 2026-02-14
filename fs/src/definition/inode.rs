use crate::definition::SuperBlock;

/// The definition of the file type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    /// Reserved data
    pub _reserved: [u8; 8],
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

        /// Create an inode object by a slice.
    /// 
    /// # Parameters
    /// 
    /// * `buf` - The slice of bytes.
    /// 
    /// # Returns
    /// 
    /// * `Self` - The inode object.
    pub fn from_bytes(buf: &[u8]) -> Option<&Self> {
        if buf.len() < core::mem::size_of::<Self>() {
            return None;
        }
        let inode = unsafe {
            &*(buf[0] as *const u8 as *const Self)
        };
        Some(inode)
    }

    /// Create this inode object by a mutable slice.
    /// 
    /// # Parameters
    /// 
    /// * `buf` - The slice of bytes.
    /// 
    /// # Returns
    /// 
    /// * `Self` - The inode object.
    pub fn from_bytes_mut(buf: &mut [u8]) -> Option<&mut Self> {
        if buf.len() < core::mem::size_of::<Self>() {
            return None;
        }
        let inode = unsafe {
            &mut *(buf[0] as *mut u8 as *mut Self)
        };
        Some(inode)
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
