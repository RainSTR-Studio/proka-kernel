//! The driver process definition.
extern crate alloc;
use super::{Context, Error, MAX_PS, Status};
use alloc::{vec, vec::Vec};
use spin::{LazyLock, RwLock};

pub static DRIVER_PROCESS: LazyLock<RwLock<DriverProcessTable>> =
    LazyLock::new(|| RwLock::new(DriverProcessTable::new()));

/// The driver process list.
#[repr(C)]
#[derive(Debug)]
pub struct DriverProcessTable {
    pub process: Vec<DriverProcess>,
    pub count: u16,
}

impl DriverProcessTable {
    pub fn new() -> Self {
        let process = vec![DriverProcess::default(); MAX_PS];
        Self { process, count: 0 }
    }
}

impl Default for DriverProcessTable {
    fn default() -> Self {
        Self::new()
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
    pub context: Context,

    /// The bottom of the stack.
    pub stack_bottom: u64,

    /// Current heap bottom.
    pub heap_bottom: u64,

    /// Current heap top.
    pub heap_top: u64,

    /// The process's page table.
    pub table_addr: u64,
}

impl DriverProcess {
    /// Create a process.
    #[inline]
    pub fn create(frame: u64, stack_size: u64) -> Result<Self, Error> {
        Ok(Self {
            present: true,
            status: Status::Ready,
            context: Context::driver(),
            stack_bottom: Context::driver().rsp - stack_size,
            heap_top: 0x180000000,
            heap_bottom: 0x180000000,
            table_addr: frame,
        })
    }

    /// Remove this process.
    #[inline]
    pub fn remove(&mut self) {
        self.present = false;
    }
}
