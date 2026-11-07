//! The `syscall`/`sysret` fast path of the SDMS subsystem.
//!
//! The setup half of the fast path (STAR/LSTAR/SFMask) lives in the parent
//! module as [`super::init_reg`]; this file only owns the entry stub and the
//! call dispatch itself.
extern crate alloc;
use core::arch::{asm, naked_asm};
use proka_kernel_core::sdms::SDMSTABLE;

/// The syscall common entry.
///
/// # Arguments
/// - RAX: The syscall number
/// - RDI: The syscall arg 1
/// - RSI: The syscall arg 2
/// - RDX: The syscall arg 3
/// - R8: The syscall arg 4
/// - R9: The syscall arg 5
/// - R12: The syscall arg 6 (always buffer addr)
/// - R13: The syscall arg 7 (always buffer size)
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry() {
    naked_asm!(
        // Save and update RBP
        "push rbp",
        "mov rbp, rsp",
        // Push all of the registers (no RAX)
        "cli",
        "push rbx",
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
        // Save and switch stack and table.
        "mov r14, rsp",
        "mov r15, cr3",
        "mov r13, 0x100000",
        "mov cr3, r13",
        "mov rsp, 0xffff80004007f000",
        // Save return info
        "push r14",
        "push r15",
        /* Alloc temporary stack */
        // Save arguments first
        "push rax",
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        "push r12",
        "push r13",
        // Call stack allocator
        "mov rdi, r15",
        "call alloc_tempstack",
        // Restore register value
        "pop r13",
        "pop r12",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r13", // push rax
        "pop r15",
        "pop r14",
        // Switch stack and restore sysnum
        "mov rsp, rax",
        "mov rax, r13", // Move back sysnum
        "push r14",
        "push r15",
        // Convert our own origin ABI to System V ABI
        "mov rcx, r8",
        "mov r8, r9",
        // Enter main function
        "sti",
        "call syscall_handler",
        "cli",
        // Recover original stack and table.
        "pop r15",
        "pop r14",
        /* Deallocate temporary stack */
        // Move to temporary stack
        "mov r13, rsp", // Get current stack top
        "mov rsp, 0xffff80004007fff0",
        "mov rdi, r13", // Arg #1 (stack_top)
        "mov rsi, r15", // Arg #2 (target_pml4)
        "push r14",
        "push r15",
        "call dealloc_tempstack",
        "pop r15",
        "pop r14",
        // Recover origin stack and table
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
        "pop rdx",
        "pop rcx",
        "pop rbx",
        // Pop RBP and return
        "sti",
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
    let buf_addr: u64;
    let buf_size: u64;
    unsafe {
        asm!(
            "nop",
            out("rax") call_num,
            out("r12") buf_addr,
            out("r13") buf_size,
            out("r15") user_table,
        )
    }

    // Look the syscall up in the normal-process half of the SDMS table.
    // The entry is copied out and the guard is dropped before the switch,
    // so a syscall is free to touch `SDMSTABLE` itself.
    let found = {
        let table = SDMSTABLE.read();
        table
            .sms_entry
            .iter()
            .find(|entry| entry.sysnum == call_num)
            .map(|entry| (entry.entry, entry.page_table))
    };
    let Some((entry, page_table)) = found else {
        return -1;
    };

    // If the user passed bufaddr/bufsize isn't empty, we
    // should map it to the current syscall process's space.
    let dbuf_addr = map_to_desc_syscall(buf_addr, buf_size);

    // Move to target table
    unsafe {
        asm!(
            "mov cr3, {}",
            in(reg) page_table,
            in("r15") user_table,
        )
    }

    let result = entry(arg1, arg2, arg3, arg4, arg5, dbuf_addr, buf_size);

    // Demap the target syscall address and return
    demap(dbuf_addr, buf_size);
    result as i64
}

/// Map the source's buffer to driver.
///
/// # Input
///  - addr: the source address
///  - size: the buffer size.
///
/// # Output
///  - dst_addr: the dest process's address.
fn map_to_desc_syscall(_buf_addr: u64, _buf_size: u64) -> u64 {
    // TODO: Impl this feature
    0
}

/// Reverse of map_to_desc_syscall.
///
/// # Input
///  - dst_addr: the destination syscall process buf addr
///  - size: the buffer size
fn demap(_dst_addr: u64, _size: u64) {}
