//! SDMS - x86_64 implementation of the merged syscall / driver-call subsystem.
//!
//! Two hardware paths land here:
//!  - `sms`: the `syscall`/`sysret` fast path, set up through the
//!    STAR/LSTAR/SFMask MSRs and dispatched from [`syscall_entry`];
//!  - `dms`: the `int 0x40` slow path, dispatched from [`coredrv`].
//!
//! Both share `proka_kernel_core::sdms::SDMSTABLE`, differing only in which
//! half of the table (`sms_entry` / `dms_entry`) they look the call number up in.
pub mod dms;
pub mod memory;
pub mod sms;
use crate::handler::syscall_entry;
use crate::tables::gdt::GDT;
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
/// call table lives in `proka_kernel_core::sdms`.
pub fn init_reg() {
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
