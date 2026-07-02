//! The syscall initializator.
extern crate alloc;
use alloc::vec::Vec;
use spin::Mutex;
use x86_64::{VirtAddr, registers::model_specific::{LStar, Star}};
use crate::{handler::syscall_entry, tables::gdt::GDT};

/// The syscall number manager.
pub static SYSCALL: Mutex<Vec<SyscallEntry>> = {
    let syscalls = Vec::new();
    Mutex::new(syscalls)
};

/// The syscall entry.
pub struct SyscallEntry {
    /// The syscall number.
    pub sysnum: u64,

    /// The dest page table.
    ///
    /// This system will automatically switch into this 
    /// table and pass the arguments...
    pub page_table: u64,

    /// The dest entry point addr after switching table.
    pub entry: u64,
}

pub fn init() {
    // Firstly, update STAR registers
    let sel = GDT.1;
    Star::write(
        sel.user_code,
        sel.user_data,
        sel.kernel_code,
        sel.kernel_data,
    )
    .expect("Failed to do STAR register writing");

    // Then update LSTAR.
    let addr = syscall_entry as *const () as u64;
    LStar::write(VirtAddr::new(addr));
}
