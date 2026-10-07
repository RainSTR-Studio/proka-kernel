//! x86_64 specific syscall initializator.
pub mod memory;
pub mod power;
pub mod process;

use crate::{handler::syscall_entry, tables::gdt::GDT};
use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{LStar, SFMask, Star},
        rflags::RFlags,
    },
};

/// Initialize the x86_64 MSR-based syscall entry (STAR/LSTAR/SFMask).
///
/// This is the architecture-specific half of syscall setup; the generic
/// syscall registration table lives in `proka_common::syscall`.
pub fn init_msr() {
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
}
