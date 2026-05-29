//! The driver process definition.
extern crate alloc;
use super::{Context, Error, MAX_PS, Status};
use alloc::{vec, vec::Vec};
use spin::{Lazy, Mutex};

pub static DRIVER_PROCESS: Lazy<Mutex<DriverProcessTable>> =
    Lazy::new(|| Mutex::new(DriverProcessTable::default()));

/// The driver process list.
#[repr(C)]
#[derive(Debug)]
pub struct DriverProcessTable {
    pub process: Vec<DriverProcess>,
    pub count: u16,
}

impl DriverProcessTable {
    pub fn default() -> Self {
        let process = vec![DriverProcess::default(); MAX_PS];
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
    pub context: Context,

    /// The process's page table.
    pub table_addr: u64,
}

impl DriverProcess {
    /// Create a process.
    #[inline]
    pub fn create(frame: u64) -> Result<Self, Error> {
        Ok(Self {
            present: true,
            status: Status::Ready,
            context: Context::default(),
            table_addr: frame,
        })
    }

    /// Remove this process.
    #[inline]
    pub fn remove(&mut self) {
        self.present = false;
    }
}
