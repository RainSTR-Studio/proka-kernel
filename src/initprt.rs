//! The INITPRT parser.
extern crate alloc;
use crate::memory::framealloc::FRAME_ALLOCATOR;
use alloc::format;
use alloc::string::String;
use alloc::{vec, vec::Vec};
use hadris_fat::{Error, ErrorKind, FatDir, FatFs, IoResult, Read, Seek, SeekFrom};
use log::debug;
use proka_exec::{Parser, header::ExecMode};
use serde::Deserialize;
use x86_64::structures::paging::PhysFrame;
use x86_64::{PhysAddr, align_up};

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
    let list_content = load(&drivers, "list.toml", false);
    let lists: DrvList =
        toml::from_slice(&list_content.2).expect("Failed to parse drivers list.toml");
    for driver in lists.drivers {
        #[cfg(debug_assertions)]
        debug!("Driver: {}", driver);
        run(&drivers, &driver, ExecMode::CoreDrv);
    }
}

/// Load proka exec file as the normal process program.
/// Returns: (addr, size, buf)
fn load<'a>(dir: &FatDir<'_, InitprtReader>, file: &str, is_exec: bool) -> (u64, u64, Vec<u8>) {
    // Open file...
    let mut init = dir
        .open_file(file)
        .expect(&format!("Failed to load {}", file));
    let size = init.size();

    // Construct a slice to contain that executable
    let mut buf = if is_exec {
        let pages = (align_up(size as u64, 4096) >> 12) as usize;
        let base = FRAME_ALLOCATOR
            .lock()
            .allocate_contiguous(pages)
            .expect("Failed to alloc a frame to store data");
        let addr = base.start_address().as_u64();
        debug!("Init will put into 0x{:08x}", addr);
        unsafe { core::slice::from_raw_parts_mut(addr as *mut u8, size) }.to_vec()
    } else {
        vec![0u8; size]
    };

    // Read!
    init.read(&mut buf).unwrap();
    (buf.as_ptr() as u64, size as u64, buf)
}

fn run(dir: &FatDir<'_, InitprtReader>, file: &str, mode: ExecMode) {
    // Get buffer
    let info = load(dir, file, true);
    let buf = &info.2;

    // Temporary initialize parser to check is mode correct
    // SAFETY: buffer already mapped
    {
        let parser = Parser::init(&buf).expect("{} is corrupted");
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
    unsafe { crate::process::create(&buf, 0).unwrap() }

    // Drop the buffer's region
    // Idea: Deallocate that region
    let pages = (align_up(info.1, 4096) >> 12) as usize;
    FRAME_ALLOCATOR
        .lock()
        .deallocate_contiguous(PhysFrame::containing_address(PhysAddr::new(info.0)), pages);
}
