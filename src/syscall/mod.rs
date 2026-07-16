//! The syscall module.
extern crate alloc;
pub mod power;
pub mod process;
use crate::{handler::syscall_entry, tables::gdt::GDT};
use alloc::vec::Vec;
use spin::RwLock;
use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{LStar, SFMask, Star},
        rflags::RFlags,
    },
};

/// The syscall number manager.
pub static SYSCALL: RwLock<Vec<SyscallEntry>> = {
    let syscalls = Vec::new();
    RwLock::new(syscalls)
};

/// The syscall return type.
#[repr(C)]
pub enum ReturnType {
    /// Returns success with a number.
    Success(i64),

    /// Returns error with specified error num.
    Error(i32),
}

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
    pub entry: extern "C" fn(u64, u64, u64, u64, u64) -> ReturnType,
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

    // Finally write SFMask
    SFMask::write(RFlags::DIRECTION_FLAG);

    // It's time to register kernel's own syscall...
    // For syscall 1 (power action)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 1,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: power::power,
    });
}
