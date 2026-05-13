//! The INITPRT parser.
use crate::println;
use hadris_fat::{Error, ErrorKind, FatFs, Read, Seek, SeekFrom};

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

// Read
impl Read for InitprtReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
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
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error> {
        let len = INITPRT_LENGTH as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => {
                let off = offset as i64;
                if off < 0 || off > len {
                    return Err(Error::new(ErrorKind::Other, "seek position out of bounds"));
                }
                off
            }
            SeekFrom::End(offset) => {
                let off = len + offset;
                if off < 0 || off > len {
                    return Err(Error::new(ErrorKind::Other, "seek position out of bounds"));
                }
                off
            }
            SeekFrom::Current(offset) => {
                let off = self.pos as i64 + offset;
                if off < 0 || off > len {
                    return Err(Error::new(ErrorKind::Other, "seek position out of bounds"));
                }
                off
            }
        };

        self.pos = new_pos as usize;
        Ok(new_pos as u64)
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
    let fs = FatFs::open(reader).expect("Failed to load initprt");
    let root = fs.root_dir();
    let mut iter = root.entries();
    while let Some(Ok(entry)) = iter.next_entry() {
        println!("{}", entry.name());
    }
}
