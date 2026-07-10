//! The INITPRT parser.
extern crate alloc;
use alloc::string::String;
use alloc::{vec, vec::Vec};
use hadris_fat::{Error, ErrorKind, FatDir, FatFs, IoResult, Read, Seek, SeekFrom};
#[cfg(debug_assertions)]
use log::debug;
use log::warn;
use proka_exec::{Parser, header::ExecMode};
use serde::Deserialize;

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
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
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
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let len = INITPRT_LENGTH as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => {
                let off = offset as i64;
                if off < 0 || off > len {
                    return Err(Error::new(ErrorKind::Other, "seek out of bounds"));
                }
                off
            }
            SeekFrom::End(offset) => {
                let off = len + offset;
                if off < 0 || off > len {
                    return Err(Error::new(ErrorKind::Other, "seek out of bounds"));
                }
                off
            }
            SeekFrom::Current(offset) => {
                let off = self.pos as i64 + offset;
                if off < 0 || off > len {
                    return Err(Error::new(ErrorKind::Other, "seek out of bounds"));
                }
                off
            }
        };

        self.pos = new_pos as usize;
        Ok(new_pos as u64)
    }
}

/// The format of `/drivers/list.toml`.
#[derive(Debug, Clone, Deserialize)]
struct DrvList {
    pub drivers: Vec<String>,
}

/*
 * Then, the functions begins...
 */

/// Load all of the file which is in initprt and essential.
pub fn init() {
    // Init fs
    let reader = InitprtReader::init();
    let fs = FatFs::open(reader).expect("Failed to load initprt");

    // Load init, as userapp mode...
    let root = fs.root_dir();
    run(&root, "init", ExecMode::UserApp);

    // Then, parse the `/drivers/list.toml`.
    let drivers = fs
        .open_dir_path("drivers")
        .expect("Failed to load driver path");
    let list_content = load(&drivers, "list.toml");
    let lists: DrvList =
        toml::from_slice(&list_content.2).expect("Failed to parse drivers list.toml");

    // Check: Is driver list empty
    if lists.drivers.is_empty() {
        warn!("The driver list is empty, no drivers will be run!!");
        return;
    }

    // Iterate drivers and run them
    for driver in lists.drivers {
        #[cfg(debug_assertions)]
        debug!("Driver: {}", driver);
        run(&drivers, &driver, ExecMode::CoreDrv);
    }
}

/// Load proka exec file as the normal process program.
/// Returns: (addr, size, buf)
fn load(dir: &FatDir<'_, InitprtReader>, file: &str) -> (u64, u64, Vec<u8>) {
    // Open file...
    let mut init = dir
        .open_file(file)
        .unwrap_or_else(|_| panic!("Failed to load {}", file));
    let size = init.size();

    // Construct a slice to contain that executable
    let mut buf = vec![0u8; size];
    // Read!
    init.read(&mut buf).unwrap();
    (buf.as_ptr() as u64, size as u64, buf)
}

fn run(dir: &FatDir<'_, InitprtReader>, file: &str, mode: ExecMode) {
    // Get buffer
    let info = load(dir, file);
    let buf = &info.2;

    // Temporary initialize parser to check is mode correct
    // SAFETY: buffer already mapped
    {
        let parser = Parser::init(buf).expect("{} is corrupted");
        let filemode = parser.header().mode;
        if filemode != mode {
            panic!(
                "The mode of {} is incorrect (expected \"{:?}\", found \"{:?}\")",
                file, mode, filemode
            );
        }
    }

    // Then create up a process
    // TODO: Turn panic into internal shell
    // SAFETY: buffer already mapped and read
    unsafe { crate::process::create(buf, 0).unwrap() }
}
