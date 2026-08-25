//! The normal process definition.
extern crate alloc;
use super::{Context, Error, MAX_PS, Status};
use alloc::{vec, vec::Vec};
use spin::{LazyLock, RwLock};

pub static NORMAL_PROCESS: LazyLock<RwLock<NormalProcessTable>> =
    LazyLock::new(|| RwLock::new(NormalProcessTable::new()));

/// The normal process list.
#[repr(C)]
#[derive(Debug)]
pub struct NormalProcessTable {
    pub process: Vec<NormalProcess>,
    pub count: u16,
}

impl NormalProcessTable {
    pub fn new() -> Self {
        let process = vec![NormalProcess::default(); MAX_PS];
        Self { process, count: 0 }
    }
}

impl Default for NormalProcessTable {
    fn default() -> Self {
        Self::new()
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

    /// The page table which is currently using.
    pub current_table: u64,

    /// The stack bottom address.
    pub stack_bottom: u64,

    /// The heap bottom address.
    pub heap_bottom: u64,

    /// The heap top address.
    pub heap_top: u64,

    /// The process's page table.
    pub table_addr: u64,
}

impl NormalProcess {
    /// Create a process.
    #[inline]
    pub fn create(frame: u64, priority: u8, stack_size: u64) -> Result<Self, Error> {
        Ok(Self {
            present: true,
            status: Status::Ready,
            priority,
            context: Context::normal(),
            current_table: frame,
            stack_bottom: Context::normal().rsp - stack_size,
            heap_top: 0x180000000,
            heap_bottom: 0x180000000,
            table_addr: frame,
        })
    }

    /// Remove a process.
    #[inline]
    pub fn remove(&mut self) {
        self.present = false;
    }
}
