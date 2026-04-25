//! The normal process definition.
use super::{Status, MAX_PS};
use lazy_static::lazy_static;
use spin::Mutex;

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
        let process = 
            unsafe { core::slice::from_raw_parts_mut(0xffff800000c00000 as *mut NormalProcess, MAX_PS) };
        Self {
            process,
            count: 0,
        }
    }
}

/// One process's info list.
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct NormalProcess {
    /// Assign is the process present.
    pub present: bool,

    /// The process entry point.
    pub entry: u64,

    /// The process status.
    pub status: Status,

    /// The process stack pointer.
    pub rsp: u64,

    /// The process priority.
    pub priority: u8,

    /// The process's page table.
    pub table_addr: u64,
}
