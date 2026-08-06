//! The syscall handler.
extern crate alloc;
use crate::syscall::SYSCALL;
use core::arch::{asm, naked_asm};

/// The syscall common entry.
///
/// # Arguments
/// - RAX: The syscall number
/// - RDI: The syscall arg 1
/// - RSI: The syscall arg 2
/// - RDX: The syscall arg 3
/// - R8: The syscall arg 4
/// - R9: The syscall arg 5
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry() {
    naked_asm!(
        // Save and update RBP
        "push rbp",
        "mov rbp, rsp",
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
        // Convert our own origin ABI to System V ABI, as the RCX has been overwrited.
        "mov rcx, r8",
        "mov r8, r9",
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
        // Pop RBP and return
        "pop rbp",
        "sysretq",
    );
}

/// The syscall handler.
///
/// # Arguments
///  - RAX: The syscall number
///  - RDI, RSI, RDX, R8, R9: The syscall args 1-5 (System V ABI)
#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64 {
    // Get the syscall number.
    let call_num: u64;
    let user_table: u64;
    unsafe { asm!("nop", out("rax") call_num, out("r15") user_table) }

    // Get the syscall from syscall table.
    let binding = SYSCALL.read();

    let (entry, stack, page_table) = {
        let syscall = binding.iter().find(|s| s.sysnum == call_num);

        // Check: Is this result none
        if syscall.is_none() {
            return -1;
        }

        // Now we can get the exact table safely.
        let syscall = syscall.expect("Shouldn't appear!"); // Won't panick!
        (syscall.entry, syscall.stack, syscall.page_table)
    };

    // Since we get return value, we can write to registers and return...
    let result: u64;
    unsafe {
        asm!(
            "mov cr3, {table}",
            "push rbp",
            "mov rbp, rsp",     // Use RBP to save the original stack address
            "mov rsp, {stack}",
            "push rbp",         // Save to new stack #1
            "mov r15, {user_table}",
            "call {entry}",
            "pop rsp",          // Restore directly  #1
            "pop rbp",
            entry = in(reg) entry,
            stack = in(reg) stack,
            table = in(reg) page_table,
            user_table = in(reg) user_table,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            in("rcx") arg4,
            in("r8") arg5,
            out("rax") result,
        )
    }

    result as i64
}
