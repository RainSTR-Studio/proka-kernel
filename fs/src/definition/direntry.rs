
/// The entry point of directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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