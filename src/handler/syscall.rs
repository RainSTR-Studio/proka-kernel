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
pub extern "C" fn syscall_handler() {
    // First, save the user's page table.
    let user_table: u64;
    let user_stack: u64;
    let sysnum: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;
    let arg4: u64;
    let arg5: u64;

    unsafe {
        asm!(
            "mov r12, cr3",
            "mov rdx, rsp",
            out("r12") user_table,
            out("rdx") user_stack,
            out("rax") sysnum,
            out("rcx") _,
            out("rdi") arg1,
            out("rsi") arg2,
            out("r8") arg3,
            out("r9") arg4,
            out("r10") arg5,
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
            // Save info
            "mov rbx, {0}",
            "mov r13, {1}",
            "mov rsp, 0xffff80004007f000",
            "mov cr3, {2}",
            "push rbx",
            "push r13",
            // Push arg registers
            "push rdi",
            "push rsi",
            "push r8",
            "push r9",
            "push r10",
            // Call main fn
            "call {3}",
            // Pop arg one...
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rsi",
            "pop rdi",
            // Pop essential registers
            "pop r13",
            "pop rbx",
            "mov cr3, r13",
            "mov rsp, rbx",
            in(reg) user_stack,
            in(reg) user_table,
            in(reg) entry.page_table,
            in(reg) entry.entry,
            in("rdi") arg1,
            in("rsi") arg2,
            in("r8") arg3,
            in("r9") arg4,
            in("r10") arg5,
            out("r11") _,
            out("rcx") _,
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
