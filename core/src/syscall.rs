//! Kernel syscall basic definitions.
extern crate alloc;
use alloc::vec::Vec;
use spin::RwLock;

/// The syscall number manager.
pub static SYSCALL: RwLock<Vec<SyscallEntry>> = {
    let syscalls = Vec::new();
    RwLock::new(syscalls)
};

/// The syscall entry.
#[derive(Debug, Clone, Copy)]
pub struct SyscallEntry {
    /// The syscall number.
    pub sysnum: u64,

    /// The dest page table.
    ///
    /// This system will automatically switch into this
    /// table and pass the arguments...
    pub page_table: u64,

    /// The dest RSP address (stack top).
    pub stack: u64,

    /// The dest entry point addr after switching table.
    pub entry: extern "C" fn(u64, u64, u64, u64, u64) -> i64,
}
