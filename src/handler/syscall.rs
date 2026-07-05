//! The syscall handler.
use core::arch::naked_asm;

/// The syscall common entry.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry() {
    naked_asm!(
        // Push all of the registers
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsp",
        "push rbp",
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
        // Enter main function
        "call syscall_handler"
    );
}

/// The syscall handler.
#[unsafe(no_mangle)]

pub fn syscall_handler() {}
