//! The driver process definition.
extern crate alloc;
use alloc::boxed::Box;
use crate::memory::framealloc::FRAME_ALLOCATOR;
use super::{Status, MAX_PS, Error, Context};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::paging::FrameAllocator;

// Constants
/// The RSP of drivers.
pub const DRIVER_RSP: u64 = 0x1F0000;

lazy_static! {
    pub static ref DRIVER_PROCESS: Mutex<DriverProcessTable> =
        Mutex::new(DriverProcessTable::default());
}

/// The driver process list.
#[repr(C)]
#[derive(Debug)]
pub struct DriverProcessTable {
    pub process: &'static mut [DriverProcess],
    pub count: u16,
}

impl DriverProcessTable {
    pub fn default() -> Self {
        let process = unsafe {
            core::slice::from_raw_parts_mut(0xffff800000e00000 as *mut DriverProcess, MAX_PS)
        };
        Self { process, count: 0 }
    }
}

/// One process's info list.
#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct DriverProcess {
    /// Assign is the current process exists.
    pub present: bool,

    /// The process status.
    pub status: Status,

    /// The process context.
    pub context: Box<Context>,

    /// The process's page table.
    pub table_addr: u64,
}

impl DriverProcess {
    /// Create a process.
    #[inline]
    pub fn create() -> Result<Self, Error> {
        let frame = if let Some(frame) = FRAME_ALLOCATOR.lock().allocate_frame() {
            frame.start_address().as_u64()
        } else {
            return Err(Error::MemoryNotEnough);
        };
        Ok(Self {
            present: true,
            status: Status::Ready,
            context: Box::new(Context::default()),
            table_addr: frame,
        })
    }

    /// Remove this process.
    #[inline]
    pub fn remove(&mut self) {
        self.present = false;
    }
}
