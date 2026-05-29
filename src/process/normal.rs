//! The normal process definition.
extern crate alloc;
use super::{Context, Error, MAX_PS, Status};
use alloc::{vec, vec::Vec};
use spin::{Lazy, Mutex};

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
    pub context: Context,

    /// The process's page table.
    pub table_addr: u64,
}

impl NormalProcess {
    /// Create a process.
    #[inline]
    pub fn create(frame: u64, priority: u8) -> Result<Self, Error> {
        Ok(Self {
            present: true,
            status: Status::Ready,
            priority,
            context: Context::default(),
            table_addr: frame,
        })
    }

    /// Remove a process.
    #[inline]
    pub fn remove(&mut self) {
        self.present = false;
    }
}
