//! The normal process definition.
extern crate alloc;
use alloc::boxed::Box;
use super::{Error, Status, Context, MAX_PS};
use crate::memory::framealloc::FRAME_ALLOCATOR;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::paging::FrameAllocator;

// Constants
/// RSP address for all normal process.
pub const NORMAL_RSP: u64 = 0x1FF000;

lazy_static! {
    pub static ref NORMAL_PROCESS: Mutex<NormalProcessTable> =
        Mutex::new(NormalProcessTable::default());
}

/// The normal process list.
#[repr(C)]
#[derive(Debug)]
pub struct NormalProcessTable {
    pub process: &'static mut [NormalProcess],
    pub count: u16,
}

impl NormalProcessTable {
    pub fn default() -> Self {
        let process = unsafe {
            core::slice::from_raw_parts_mut(0xffff800000c00000 as *mut NormalProcess, MAX_PS)
        };
        Self { process, count: 0 }
    }
}

/// One process's info list.
#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct NormalProcess {
    /// Assign is the process present.
    pub present: bool,

    /// The process status.
    pub status: Status,

    /// The process priority.
    pub priority: u8,

    /// The process context.
    pub context: Box<Context>,

    /// The process's page table.
    pub table_addr: u64,
}

impl NormalProcess {
    /// Create a process.
    #[inline]
    pub fn create(priority: u8) -> Result<Self, Error> {
        let frame = if let Some(frame) = FRAME_ALLOCATOR.lock().allocate_frame() {
            frame.start_address().as_u64()
        } else {
            return Err(Error::MemoryNotEnough);
        };

        Ok(Self {
            present: true,
            status: Status::Ready,
            priority,
            context: Box::new(Context::default()),
            table_addr: frame,
        })
    }

    /// Remove a process.
    #[inline]
    pub fn remove(&mut self) {
        self.present = false;
    }
}
