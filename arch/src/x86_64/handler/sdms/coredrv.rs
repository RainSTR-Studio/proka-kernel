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

/// The kernel page table the driver-call handler runs under.
const KERNEL_PAGE_TABLE: u64 = 0x100000;

/// The driver-call entry (IDT vector `0x40`).
///
/// # Register layout (set up by the driver before `int 0x40`)
///  - RAX: the driver-call number
///  - RDI: the call arg 1
///  - RSI: the call arg 2
///
/// # Stack arrangement
/// On entry (64-bit mode, vector without an error code) the CPU has pushed
/// `RIP, CS, RFLAGS, RSP, SS`: five quadwords. The CPU aligns `RSP` to 16
/// bytes before pushing, so it is congruent to 8 modulo 16 here. The nine
/// pushes below restore the 16-byte alignment the `call` requires.
#[unsafe(naked)]
pub extern "C" fn coredrv() {
    naked_asm!(
        // Preserve the caller-saved registers; `extern "C"` preserves the rest.
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        // Reorder into the System V argument registers: (num, arg1, arg2).
        "mov rdx, rsi",
        "mov rsi, rdi",
        "mov rdi, rax",
        "call coredrv_handler",
        // Restore the interrupted context.
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
    );
}

/// The `extern "C"` body of [`coredrv`].
///
/// # Arguments
///  - `call_num`: the driver-call number (was in RAX)
///  - `arg1`: the call arg 1 (was in RDI)
///  - `arg2`: the call arg 2 (was in RSI)
///
/// The stub above reaches this function through its raw symbol, hence
/// `no_mangle`.
#[unsafe(no_mangle)]
pub extern "C" fn coredrv_handler(call_num: u64, arg1: u64, arg2: u64) {
    // Switch to the kernel table first: the driver list is only reachable
    // from the kernel address space. Remember the caller's table for the
    // restore at the end.
    let caller_table = Cr3::read().0.start_address().as_u64();
    switch_table(KERNEL_PAGE_TABLE);

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
