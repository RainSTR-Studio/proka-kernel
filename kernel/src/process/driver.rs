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
#[derive(Debug)]
pub struct DriverProcessTable {
    pub process: &'static mut [DriverProcess],
    pub count: u16,
}

impl DriverProcessTable {
    pub fn default() -> Self {
        let process = 
            unsafe { core::slice::from_raw_parts_mut(0xffff800000e00000 as *mut DriverProcess, MAX_PS) };
        Self {
            process,
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
