//! This code is originally from https://github.com/rcore-os/cpio/blob/main/src/lib.rs, with minor modifications made here.

extern crate alloc;
use crate::fs::fs_impl::MemFs;
use crate::fs::vfs::{File, FileSystem, VNodeType, VfsError, VFS};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use log::{debug, error, info, warn};

/// A CPIO file (newc format) reader.
///
/// # Example
///
/// ```rust,should_panic
/// use cpio::CpioNewcReader;
///
/// let reader = CpioNewcReader::new(&[]);
/// for obj in reader {
///     println!("{}", obj.unwrap().name);
/// }
/// ```
pub struct CpioNewcReader<'a> {
    buf: &'a [u8],
}

impl<'a> CpioNewcReader<'a> {
    /// Creates a new CPIO reader on the buffer.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}

/// File system object in CPIO file.
pub struct Object<'a> {
    /// The file metadata.
    pub metadata: Metadata,
    /// The full pathname.
    pub name: &'a str,
    /// The file data.
    pub data: &'a [u8],
}

impl<'a> Iterator for CpioNewcReader<'a> {
    type Item = Result<Object<'a>, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: To workaround lifetime
        let s: &'a mut Self = unsafe { core::mem::transmute(self) };
        match inner(&mut s.buf) {
            Ok(Object {
                name: "TRAILER!!!", ..
            }) => None,
            res => Some(res),
        }
    }
}

fn inner<'a>(buf: &'a mut &'a [u8]) -> Result<Object<'a>, ReadError> {
    const HEADER_LEN: usize = 110;
    const MAGIC_NUMBER: &[u8] = b"070701";

    if buf.len() < HEADER_LEN {
        return Err(ReadError::BufTooShort);
    }
    let magic = buf.read_bytes(6)?;
    if magic != MAGIC_NUMBER {
        return Err(ReadError::InvalidMagic);
    }
    let ino = buf.read_hex_u32()?;
    let mode = buf.read_hex_u32()?;
    let uid = buf.read_hex_u32()?;
    let gid = buf.read_hex_u32()?;
    let nlink = buf.read_hex_u32()?;
    let mtime = buf.read_hex_u32()?;
    let file_size = buf.read_hex_u32()?;
    let dev_major = buf.read_hex_u32()?;
    let dev_minor = buf.read_hex_u32()?;
    let rdev_major = buf.read_hex_u32()?;
    let rdev_minor = buf.read_hex_u32()?;
    let name_size = buf.read_hex_u32()? as usize;
    let _check = buf.read_hex_u32()?;
    let metadata = Metadata {
        ino,
        mode,
        uid,
        gid,
        nlink,
        mtime,
        file_size,
        dev_major,
        dev_minor,
        rdev_major,
        rdev_minor,
    };
    let name_with_nul = buf.read_bytes(name_size)?;
    if name_with_nul.last() != Some(&0) {
        return Err(ReadError::InvalidName);
    }
    let name = core::str::from_utf8(&name_with_nul[..name_size - 1])
        .map_err(|_| ReadError::InvalidName)?;
    buf.read_bytes(pad_to_4(HEADER_LEN + name_size))?;

    let data = buf.read_bytes(file_size as usize)?;
    buf.read_bytes(pad_to_4(file_size as usize))?;

    Ok(Object {
        metadata,
        name,
        data,
    })
}

trait BufExt<'a> {
    fn read_hex_u32(&mut self) -> Result<u32, ReadError>;
    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ReadError>;
}

impl<'a> BufExt<'a> for &'a [u8] {
    fn read_hex_u32(&mut self) -> Result<u32, ReadError> {
        let (hex, rest) = self.split_at(8);
        *self = rest;
        let str = core::str::from_utf8(hex).map_err(|_| ReadError::InvalidASCII)?;
        let value = u32::from_str_radix(str, 16).map_err(|_| ReadError::InvalidASCII)?;
        Ok(value)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ReadError> {
        if self.len() < len {
            return Err(ReadError::BufTooShort);
        }
        let (bytes, rest) = self.split_at(len);
        *self = rest;
        Ok(bytes)
    }
}

/// pad out to a multiple of 4 bytes
fn pad_to_4(len: usize) -> usize {
    match len % 4 {
        0 => 0,
        x => 4 - x,
    }
}

/// The error type which is returned from CPIO reader.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    InvalidASCII,
    InvalidMagic,
    InvalidName,
    BufTooShort,
}

/// The file metadata.
#[derive(Debug)]
pub struct Metadata {
    pub ino: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub nlink: u32,
    pub mtime: u32,
    pub file_size: u32,
    pub dev_major: u32,
    pub dev_minor: u32,
    pub rdev_major: u32,
    pub rdev_minor: u32,
}

// CPIO mode constants
const CPIO_S_IFMT: u32 = 0o170000; // Mask for file type
const CPIO_S_IFDIR: u32 = 0o040000; // Directory
const CPIO_S_IFREG: u32 = 0o100000; // Regular file
const CPIO_S_IFLNK: u32 = 0o120000; // Symbolic link

/// Loads the initial RAM disk (initrd) into the Virtual File System (VFS).
///
/// This function parses a CPIO archive provided as raw bytes, extracts its
/// contents, and recreates the file and directory structure within the VFS.
///
/// # Arguments
/// * `initrd_data` - A byte slice containing the CPIO archive data.
///
/// # Returns
/// A `Result` indicating success or a `VfsError` if any operation fails.
pub fn load_cpio(initrd_data: &[u8]) -> Result<(), VfsError> {
    let reader = CpioNewcReader::new(initrd_data);
    let vfs = &*VFS;
    debug!("Loading CPIO archive...");
    for obj_result in reader {
        let obj = obj_result.map_err(|e| {
            error!("CPIO read error: {:?}", e);
            VfsError::IoError
        })?;

        let path = obj.name;
        if path == "TRAILER!!!" {
            continue; // Skip the trailer entry, already handled by iterator but good for explicit check
        }

        // Normalize path: CPIO paths are often like "foo/bar" or "./foo/bar".
        // We want absolute paths in VFS, e.g., "/foo/bar".
        let canonical_path = if path.starts_with('/') {
            path.to_string()
        } else if let Some(stripped) = path.strip_prefix("./") {
            format!("/{}", stripped)
        } else {
            format!("/{}", path)
        };

        // Remove trailing slash unless it's the root itself.
        let final_path = if canonical_path.len() > 1 && canonical_path.ends_with('/') {
            canonical_path.trim_end_matches('/').to_string()
        } else {
            canonical_path
        };

        let node_type_mode = obj.metadata.mode & CPIO_S_IFMT;

        // Ensure all parent directories exist for the current object's path.
        // This loop iterates through path components and creates intermediate
        // directories if they don't already exist.
        let mut current_dir_segment = String::new();
        let components: Vec<&str> = final_path.split('/').filter(|&s| !s.is_empty()).collect();

        for (i, component) in components.iter().enumerate() {
            current_dir_segment.push('/');
            current_dir_segment.push_str(component);

            // If it's an intermediate component OR the last component is a directory itself,
            // ensure it exists and is a directory.
            if i < components.len() - 1 || node_type_mode == CPIO_S_IFDIR {
                match vfs.lookup(&current_dir_segment) {
                    Ok(node) => {
                        if node.node_type() != VNodeType::Dir {
                            error!(
                                "Path component '{}' for '{}' is not a directory!",
                                current_dir_segment, final_path
                            );
                            return Err(VfsError::AlreadyExists); // Or a specific error
                        }
                    }
                    Err(VfsError::NotFound) => {
                        vfs.create_dir(&current_dir_segment).inspect_err(|e| {
                            error!(
                                "Failed to create directory {}: {:?}",
                                current_dir_segment, e
                            );
                        })?;
                    }
                    Err(e) => {
                        error!("Error checking path {}: {:?}", current_dir_segment, e);
                        return Err(e);
                    }
                }
            }
        }
        debug!("Created parent directories for {}", final_path);
        // Now, handle the actual CPIO object based on its type
        match node_type_mode {
            CPIO_S_IFREG => {
                let file_inode = vfs.create_file(&final_path)?;
                debug!("Created file {}", final_path);
                let file_handle = File::new(file_inode);
                file_handle.write(obj.data)?;
            }
            CPIO_S_IFLNK => {
                let target_path =
                    core::str::from_utf8(obj.data).map_err(|_| VfsError::InvalidArgument)?;
                vfs.create_symlink(target_path, &final_path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn load_initrd() {
    let memfs = MemFs;
    let root = memfs
        .mount(None, None)
        .expect("Failed to initialize root filesystem");
    VFS.init_root(root);

    // Load initrd
    if let Some(initrd_response) = crate::MODULE_REQUEST.get_response() {
        if let Some(inir) = initrd_response.modules().first() {
            unsafe {
                let slice: &[u8] = core::slice::from_raw_parts(inir.addr(), inir.size() as usize);
                match load_cpio(slice) {
                    Ok(_) => info!("Initrd loaded successfully."),
                    Err(e) => error!("Failed to load initrd: {:?}", e),
                }
            }
        } else {
            warn!("No initrd module found.");
        }
    } else {
        warn!("Initrd module request failed.");
    }
}
