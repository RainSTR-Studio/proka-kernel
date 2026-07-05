//! The syscall handler.
use core::arch::{asm, naked_asm};

use crate::syscall::SYSCALL;

/// The syscall common entry.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry() {
    naked_asm!(
        // Push all of the registers (no RAX)
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
        "call syscall_handler",
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
        "pop rbp",
        "pop rsp",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "sysretq",
    );
}

/// The syscall handler.
///
/// # Arguments
///  - RAX: The syscall number
///  - RDI: The syscall arg 1
///  - RSI: The syscall arg 2
///  - R8: The syscall arg 3
///  - R9: The syscall arg 4
///  - R10: The syscall arg 5
#[unsafe(no_mangle)]
pub fn syscall_handler() {
    // First, save the user's page table.
    let user_table: u64;
    let user_stack: u64;
    let sysnum: u64;

    unsafe {
        asm!(
            "mov r12, cr3",
            "mov rdx, rsp",
            out("r12") user_table,
            out("rdx") user_stack,
            out("rax") sysnum,
            out("rcx") _,
            out("rdi") _,
            out("rsi") _,
            out("r8") _,
            out("r9") _,
            out("r10") _,
            out("r11") _,
        )
    }

    // Search for syscall table
    let table = SYSCALL.lock();
    let entry = table.iter().find(|e| e.sysnum == sysnum);
    if entry.is_none() {
        sysreturn(0xffff_ffff_ffff_ffff);
        return;
    }

    let entry = entry.unwrap(); // Safety: Already asserted is `None` or `Some`.

    // Switch to process's page table, call and return
    unsafe {
        asm!(
            "mov rbx, {0}",
            "mov r13, {1}",
            "mov rsp, 0xffff80004007f000",
            "mov cr3, {2}",
            "push rbx",
            "push r13",
            "call {3}",
            "pop r13",
            "pop rbx",
            "mov cr3, rbx",
            "mov rsp, r13",
            in(reg) user_table,
            in(reg) user_stack,
            in(reg) entry.page_table,
            in(reg) entry.entry
        )
    }

    return;
}

/// Return from syscall handler.
#[inline(always)]
fn sysreturn(code: u64) {
    // Safety: Write RAX only
    unsafe {
        asm!(
            "mov rax, {0}",
            in(reg) code,
        );
    }
}
