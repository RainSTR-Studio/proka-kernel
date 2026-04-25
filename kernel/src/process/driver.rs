//! The driver process definition.
use super::{Status, MAX_PS};
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref DRIVER_PROCESS: Mutex<DriverProcessTable> =
        Mutex::new(DriverProcessTable::default());
}

/// The driver process list.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DriverProcessTable {
    pub process: [DriverProcess; MAX_PS],
    pub count: u16,
}

impl DriverProcessTable {
    pub fn default() -> Self {
        Self {
            process: [DriverProcess::default(); MAX_PS],
            count: 0,
        }
    }
}

/// One process's info list.
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct DriverProcess {
    /// Assign is the current process exists.
    pub present: bool,

    /// The process entry point.
    pub entry: u64,

    /// The process status.
    pub status: Status,

    /// The process stack pointer.
    pub rsp: u64,

    /// The process's page table.
    pub table_addr: u64,
}
