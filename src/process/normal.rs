//! The normal process definition.
extern crate alloc;
use super::{Context, Error, MAX_PS, Status};
use crate::memory::framealloc::FRAME_ALLOCATOR;
use alloc::boxed::Box;
use alloc::{vec, vec::Vec};
use spin::{Lazy, Mutex};
use x86_64::structures::paging::FrameAllocator;

pub static NORMAL_PROCESS: Lazy<Mutex<NormalProcessTable>> =
    Lazy::new(|| Mutex::new(NormalProcessTable::default()));

/// The normal process list.
#[repr(C)]
#[derive(Debug)]
pub struct NormalProcessTable {
    pub process: Vec<NormalProcess>,
    pub count: u16,
}

impl NormalProcessTable {
    pub fn default() -> Self {
        let process = vec![NormalProcess::default(); MAX_PS];
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
