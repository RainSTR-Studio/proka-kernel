//! The syscall handler.
extern crate alloc;
use crate::syscall::SYSCALL;
use core::arch::{asm, naked_asm};
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
        // Save and switch stack and table.
        "mov r14, rsp",
        "mov r15, cr3",
        "mov r13, 0x100000",
        "mov cr3, r13",
        "mov rsp, 0xffff80004007f000",
        "push r14",
        "push r15",
        // Enter main function
        "call syscall_handler",
        // Recover original stack and table.
        "pop r15",
        "pop r14",
        "mov rsp, r14",
        "mov cr3, r15",
        // Recover common registers
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
///  - RDI, RSI, RDX, R8, R9: The syscall args 1-5 (System V ABI)
#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) {
    // Get the syscall number.
    let call_num: u64;
    unsafe { asm!("nop", out("rax") call_num) }

    // Get the syscall from syscall table.
    let binding = SYSCALL.read();
    let syscall = binding.iter().find(|s| s.sysnum == call_num);

    // Check: Is this result none
    if syscall.is_none() {
        sysreturn(0xffff_ffff_ffff_ffff as u64);
        return;
    }

    // Now we can get the exact table safely.
    let syscall = syscall.expect("Shouldn't appear!"); // Won't panick!

    // Since we get return value, we can write to registers and return...
    let result: u64;
    unsafe {
        asm!(
            "push rbp",
            "mov cr3, {table}",
            "mov rsp, {stack}",
            "mov rbp, rsp",
            "mov rdi, {arg1}",
            "mov rsi, {arg2}",
            "mov rdx, {arg3}",
            "mov r8, {arg4}",
            "mov r9, {arg5}",
            "call {entry}",
            "mov rsp, r14",
            "mov cr3, r15",
            "pop rbp",
            entry = in(reg) syscall.entry,
            stack = in(reg) syscall.stack,
            table = in(reg) syscall.page_table,
            arg1 = in(reg) arg1,
            arg2 = in(reg) arg2,
            arg3 = in(reg) arg3,
            arg4 = in(reg) arg4,
            arg5 = in(reg) arg5,
            out("rax") result,
            out("r14") _,
            out("r15") _,
        )
    }

    sysreturn(result);
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
