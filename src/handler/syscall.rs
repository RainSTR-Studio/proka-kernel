//! The syscall handler.
use core::arch::naked_asm;

/// The syscall common entry.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".gdata")]
pub extern "C" fn syscall_entry() {
    naked_asm!(
        // Save all registers
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "call syscall_handler",
        // Restore all registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "sysretq",
    );
}

/// The syscall handler.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".gdata")]
pub fn syscall_handler() {}
