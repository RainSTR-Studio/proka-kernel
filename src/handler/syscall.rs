//! The syscall handler.
use core::arch::naked_asm;

/// The syscall common entry.
#[unsafe(naked)]
pub extern "C" fn syscall_entry() {
    naked_asm!(
        // TODO: Added push/pop operations.
        "call handler",
        "sysret",
    );
}

/// The syscall handler.
#[unsafe(no_mangle)]
pub fn handler() {}
