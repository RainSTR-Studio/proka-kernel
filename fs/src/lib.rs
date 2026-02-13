#![no_std]

pub mod bitmap;
pub mod definition;
pub use bitmap::Bitmap;

use crate::definition::Inode;

/// The block device driver.
pub trait BlockDevice {
    /// Read a block from the block device.
    ///
    /// # Parameters
    ///
    /// * `block_num` - The block number to read.
    /// * `buf` - The buffer to store the data.
    fn read_block(
        &mut self,
        block_num: u32,
        offset: u32,
        buf: &mut [u8],
    ) -> Result<(), &'static str>;

    /// Write a block to the block device.
    ///
    /// # Parameters
    ///
    /// * `block_num` - The block number to write.
    /// * `buf` - The data to write.
    fn write_block(&mut self, block_num: u32, offset: u32, buf: &[u8]) -> Result<(), &'static str>;
}

/// The basic structure of the whole file system.
#[repr(C)]
pub struct FileSystem<B: BlockDevice> {
    /// The block device driver.
    pub block_device: B,

    /// The super block of the file system.
    pub super_block: definition::SuperBlock,

    /// The data start block number.
    pub data_start_block: u32,
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
        let super_block = definition::SuperBlock::default();
        Self {
            block_device: bd,
            super_block: super_block,
            data_start_block: 1024, // Will dynamic calculate in the future.
        }
    }

    /// Synchronize the file system to the block device.
    pub fn sync(&mut self) -> Result<(), &'static str> {
        self.block_device
            .write_block(0, 0, &self.super_block.as_bytes())
    }

    /// Get the max inode (which means the file we can store in this fs)
    ///
    /// # Returns
    ///
    /// * `usize` - The max inode number.
    ///
    /// # Note
    ///
    /// If you want to get the max inode number, you should call this method and minus 1.
    ///
    /// # Example
    ///
    /// ```
    /// let fs = FileSystem::mount(bd);
    /// let max_inode = fs.get_max_inode();
    /// let max_inode_id = max_inode - 1;
    /// ```
    pub fn get_max_inode(&self) -> usize {
        ((self.data_start_block - 1) as usize * self.super_block.block_size as usize)
            / core::mem::size_of::<definition::Inode>()
    }

    /// Allocate an inode.
    ///
    /// # Returns
    ///
    /// * `(Inode, u32)` - The inode and the block number.
    fn alloc_inode(
        &mut self,
        file_type: definition::FileType,
    ) -> Result<(Inode, u32), &'static str> {
        // Alloc which bitmap has been used.
        let mut block_bitmap = &mut self.super_block.block_bitmap;
        let block_num = if let Some(i) = block_bitmap.alloc(128).map(|i| i as u32) {
            i
        } else {
            return Err("No block available");
        };

        // Alloc which inode has been used.
        let mut inode_bitmap = &mut self.super_block.inode_bitmap;
        let inode_num = if let Some(i) = inode_bitmap.alloc(128) {
            i as u32
        } else {
            return Err("No inode available");
        };

        // Define that inode
        let inode = Inode {
            inode_id: inode_num,
            file_type,
            head_block: block_num, // Problem: Can't sure that the behind block is free, being optimized.
            file_length: 0,
        };
        Ok((inode, block_num))
    }

    /// Create a file.
    pub fn mkfile(&mut self) -> Result<(), &'static str> {
        // 1. Allocate an inode.
        let inode_num = self.alloc_inode(definition::FileType::Regular).unwrap();

        // 2. Write the inode to the block device.
        let offset = inode_num.0.inode_id as usize * core::mem::size_of::<Inode>();
        self.block_device
            .write_block(inode_num.1, offset as u32, &inode_num.0.as_bytes())
    }

    /// Create a directory.
    pub fn mkdir(&mut self) -> Result<(), &'static str> {
        // 1. Allocate an inode.
        let inode_num = self.alloc_inode(definition::FileType::Directory).unwrap();

        // 2. Write the inode to the block device.
        let offset = inode_num.0.inode_id as usize * core::mem::size_of::<Inode>();
        self.block_device
            .write_block(inode_num.1, offset as u32, &inode_num.0.as_bytes())?;

        // 3. Create a '.' and '..' entry in the directory.
        // 3.1 Create a '.' entry.
        let name = convert_name(b".");
        let dot_dir_entry = definition::DirEntry {
            inode: inode_num.0.inode_id,
            name,
        };

        // 3.2 Create a '..' entry.
        let name = convert_name(b"..");
        let dot_dot_dir_entry = definition::DirEntry {
            inode: inode_num.0.inode_id,
            name,
        };

        // 3.3 Write the '.' and '..' entry to the block device.
        let offset = inode_num.0.inode_id as usize * core::mem::size_of::<definition::DirEntry>();
        self.block_device
            .write_block(inode_num.1, offset as u32, &dot_dir_entry.as_bytes())?;
        self.block_device.write_block(
            inode_num.1,
            (offset + core::mem::size_of::<definition::DirEntry>()) as u32,
            &dot_dot_dir_entry.as_bytes(),
        )?;
        Ok(())
    }
}

/// Convert a name to a 256 bytes array.
///
/// # Parameters
///
/// * `name_src` - The name to convert.
///
/// # Returns
///
/// * `[u8; 256]` - The converted name.
///
/// # Example
///
/// ```rust
/// let name = convert_name(b"hello");
/// ``````
pub fn convert_name(name_src: &[u8]) -> [u8; 256] {
    let mut name = [0u8; 256];
    let len = name_src.len().min(name.len() - 1);
    name[..len].copy_from_slice(&name_src[..len]);
    name
}
