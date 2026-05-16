//! The INITPRT parser.
use crate::memory::framealloc::FRAME_ALLOCATOR;
use crate::println;
use axfatfs::{Error, FileSystem, FsOptions, IoBase, Read, Seek, SeekFrom, Write};
use log::debug;
use x86_64::align_up;

// Constants
pub const INITPRT_BASE: u64 = 0xffff800003000000; // loaded
pub const INITPRT_LENGTH: usize = 0x1000000; // 16MiB

/*
 * Before we load it, we must implement a trait that read and
 * seek from that address.
 */
/// The simple initprt reader and seeker.
#[derive(Debug, Clone, Copy)]
pub struct InitprtReader {
    data: &'static [u8],
    pos: usize,
}

impl InitprtReader {
    /// Init the initprt reader.
    pub fn init() -> Self {
        let slice =
            unsafe { core::slice::from_raw_parts(INITPRT_BASE as *const u8, INITPRT_LENGTH) };

        Self {
            data: slice,
            pos: 0,
        }
    }

    /// Return is now EOF
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }
}

// IoBase
impl IoBase for InitprtReader {
    type Error = Error<()>;
}

// Read
impl Read for InitprtReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Check is it now in EOF
        if self.is_eof() {
            return Ok(0);
        }

        // Max bytes that can read
        let remain = self.data.len() - self.pos;
        let copy_len = buf.len().min(remain);

        // Copy to buffer
        buf[..copy_len].copy_from_slice(&self.data[self.pos..self.pos + copy_len]);

        // Update POS
        self.pos += copy_len;
        Ok(copy_len)
    }
}

// Seek
impl Seek for InitprtReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let len = INITPRT_LENGTH as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => {
                let off = offset as i64;
                if off < 0 || off > len {
                    return Err(Error::Io(()));
                }
                off
            }
            SeekFrom::End(offset) => {
                let off = len + offset;
                if off < 0 || off > len {
                    return Err(Error::Io(()));
                }
                off
            }
            SeekFrom::Current(offset) => {
                let off = self.pos as i64 + offset;
                if off < 0 || off > len {
                    return Err(Error::Io(()));
                }
                off
            }
        };

        self.pos = new_pos as usize;
        Ok(new_pos as u64)
    }
}

// Write (but error)
impl Write for InitprtReader {
    fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
        Err(Error::Io(()))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Err(Error::Io(()))
    }
}

/*
 * Then, the functions begins...
 */

/// Load "/init" as the normal process program.
pub fn load_init() {
    // In this fn, we just use the most simple way to load
    // the initprt's contents.
    let reader = InitprtReader::init();
    let fs = FileSystem::new(reader, FsOptions::new()).expect("Failed to load initprt");
    let root = fs.root_dir();

    for r in root.iter() {
        let entry = r.unwrap();
        println!("{}", entry.file_name());
    }

    // Open file...
    let mut init = root.open_file("/init").expect("/init not found");
    let size = {
        let mut total = 0;
        for res in init.extents() {
            if let Ok(ext) = res {
                total += ext.size;
            }
        }
        total
    };

    // Construct a slice to contain that executable
    let pages = (align_up(size as u64, 4096) >> 12) as usize;
    let base = FRAME_ALLOCATOR.lock().allocate_contiguous(pages).unwrap();
    let addr = base.start_address().as_u64();
    debug!("Init will put into {:08x}", addr);
    let buf = unsafe { core::slice::from_raw_parts_mut(addr as *mut u8, 128) };

    // Read!
    init.read(buf).unwrap();
}
