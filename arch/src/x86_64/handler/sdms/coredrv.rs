//! The driver-call (`int 0x40`) entry of the SDMS subsystem.
//!
//! Unlike the other IDT vectors, this handler does **not** use the
//! `x86-interrupt` ABI. The naked stub saves and restores the caller-saved
//! registers itself and returns with `iretq`, so this module stays free of
//! `extern "x86-interrupt"` entirely.
use crate::process::DRIVER_PROCESS;
use core::arch::naked_asm;
use proka_kernel_core::sdms::SDMSTABLE;
use x86_64::{
    PhysAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::PhysFrame,
};

/// The driver-call entry.
///
/// # Register layout (tbh it based on SysV ABI):
///  - RAX: the driver-call number
///  - RDI: the call arg 1
///  - RSI: the call arg 2
///  - RCX: the call arg 3
///  - R8: the call arg 4
///  - R9: the call arg 5
///
/// # Stack arrangement
/// On entry (64-bit mode, vector without an error code) the CPU has pushed
/// `RIP, CS, RFLAGS, RSP, SS`: five quadwords. The CPU aligns `RSP` to 16
/// bytes before pushing, so it is congruent to 8 modulo 16 here. The nine
/// pushes below restore the 16-byte alignment the `call` requires.
#[unsafe(naked)]
pub extern "C" fn coredrv() {
    naked_asm!(
        // Save and update RBP
        "push rbp",
        "mov rbp, rsp",
        // Push all of the registers
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
        // Save original rsp, original cr3
        "mov r14, rsp",
        "mov r15, cr3",
        // Switch to kernel fixed temp stack area for alloc
        "mov r13, 0x100000",
        "mov cr3, r13",
        "mov rsp, 0xffff80004004f000",
        // Save return info
        "push r14",
        "push r15",
        // Save arguments for alloc_tempstack
        "push rax",
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        "push r12",
        "push r13",
        // call alloc_tempstack(original_cr3 = r15)
        "mov rdi, r15",
        "call alloc_tempstack",
        // restore argument registers
        "pop r13",
        "pop r12",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r13", // push rax
        // Restore info registers
        "pop r15",
        "pop r14",
        // rax = allocated temp stack pointer
        "mov rsp, rax",
        "mov r13, rax",
        // Call main handler
        "sti",
        "call coredrv_handler",
        "cli",
        /* Deallocate temp stack */
        "mov r13, rsp", // current stack top
        "mov rsp, 0xffff80004004fff0",
        "mov rdi, r13", // arg1: stack_top
        "mov rsi, r15", // arg2: target_pml4
        "push r14",
        "push r15",
        "call dealloc_tempstack",
        "pop r15",
        "pop r14",
        // Restore original cr3 and original rsp
        "mov rsp, r14",
        "mov cr3, r15",
        // Restore general registers (reverse push order)
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
        "ret",
    );
}

/// The body of [`coredrv`].
///
/// The stub above reaches this function through its raw symbol, hence
/// `no_mangle`.
#[unsafe(no_mangle)]
pub extern "C" fn coredrv_handler(call_num: u64, arg1: u64, arg2: u64) {
    let caller_table: u64;
    unsafe { core::arch::asm!("mov {}, r15", out(reg) caller_table) }

    // Since we got the PML4 address, we can convert from PML4 to DID.
    let did = {
        let drvproc = DRIVER_PROCESS.read();
        drvproc
            .process
            .iter()
            .position(|process| process.table_addr == caller_table)
    };

    let Some(did) = did else {
        switch_table(caller_table);
        return;
    };

    let did = did as u16; // id is always below 16384

    // Look the call number up in the driver half of the shared table. The
    // entry is copied out so the read lock is not held across the call.
    let entry = {
        let table = SDMSTABLE.read();
        table
            .dms_entry
            .iter()
            .find(|entry| entry.sysnum == call_num)
            .copied()
    };

    // The driver id travels in the third argument slot.
    if let Some(entry) = entry {
        (entry.entry)(arg1, arg2, did as u64, 0, 0, 0, 0);
    }

    // Restore the original table.
    switch_table(caller_table);
}

/// Load `table` into CR3.
fn switch_table(table: u64) {
    // SAFETY: the caller guarantees that `table` is a valid, mapped P4
    // table that keeps the code doing the switch alive.
    unsafe {
        Cr3::write(
            PhysFrame::containing_address(PhysAddr::new(table)),
            Cr3Flags::empty(),
        )
    };
}
