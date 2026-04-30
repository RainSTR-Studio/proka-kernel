//! The scheduler.
extern crate alloc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::{
    PhysAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::{
        idt::InterruptStackFrame,
        paging::{PhysFrame, Size4KiB},
    },
};

use crate::process::{DRIVER_PROCESS, NORMAL_PROCESS};

/// The normal process queue.
pub static NORMAL_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The driver process queue.
pub static DRIVER_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Assign is it driver running or normal process.
static IS_DRIVER: AtomicBool = AtomicBool::new(true);

// Contains the current PID/DID.
static CURRENT_ID: AtomicUsize = AtomicUsize::new(0);

/// The task switcher
pub extern "x86-interrupt" fn switch_task(stack_frame: InterruptStackFrame) {
    // Switch to kernel page table IMMEDIATELY!!!
    unsafe {
        core::arch::asm!(
            "mov rax, 0x100000", // Fixed addr, safe
            "mov cr3, rax",
            options(nomem, nostack, preserves_flags)
        )
    }

    // Get smth bruh
    let normal_empty = NORMAL_QUEUE.lock().is_empty();
    let driver_empty = DRIVER_QUEUE.lock().is_empty();
    let rip = stack_frame.instruction_pointer.as_u64();
    let rsp = stack_frame.stack_pointer.as_u64();
    let rflags = stack_frame.cpu_flags.bits();

    // Check is queue is empty.
    if normal_empty && driver_empty {
        crate::apic::eoi();
        return;
    }

    // Before switching task, perhaps we shall save its basic info...
    // Get current id
    let current_id = CURRENT_ID.load(Ordering::Relaxed);

    // Decide run process or driver.
    if IS_DRIVER.load(Ordering::Relaxed) {
        IS_DRIVER.store(false, Ordering::SeqCst);

        // So let's save its RIP and RSP
        DRIVER_PROCESS.lock().process[current_id].rip = rip;
        DRIVER_PROCESS.lock().process[current_id].rsp = rsp;

        to_driver(rflags)
    } else {
        IS_DRIVER.store(true, Ordering::SeqCst);

        // Do the save step as above
        NORMAL_PROCESS.lock().process[current_id].rip = rip;
        NORMAL_PROCESS.lock().process[current_id].rsp = rsp;

        to_normal(rflags)
    };
}

// Switch to next driver.
fn to_driver(rflags: u64) {
    // Get current driver process.
    let dpt = DRIVER_PROCESS.lock();
    let mut queue = DRIVER_QUEUE.lock();

    // Get the process
    let did = queue[0];
    let proc = match dpt.process.get(did) {
        Some(p) => p,
        None => {
            crate::apic::eoi();
            return;
        }
    };

    // Check is it present
    if !proc.present {
        crate::apic::eoi();
        return;
    }

    // Save its current DID and update queue
    CURRENT_ID.store(did, Ordering::SeqCst);
    queue.remove(0);
    queue.push(did);

    // Now get its information
    let cr3 = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(proc.table_addr));
    let rsp = proc.rsp;
    let rip = proc.rip;

    // Update status and switch to target process's table
    // proc.status = Status::Running;
    unsafe {
        Cr3::write(cr3, Cr3Flags::empty());
    }

    // Update RSP and jump
    crate::apic::eoi();
    unsafe {
        core::arch::asm!(
            "push 0x10",   // SS
            "push {0}",    // RSP
            "push {1}",    // RFLAGS
            "push 0x10",   // CS, PL=0
            "push {2}",    // RIP
            "iretq",
            in(reg) rsp,
            in(reg) rflags,
            in(reg) rip,
            options(nomem, nostack, noreturn, preserves_flags)
        )
    }
}

fn to_normal(rflags: u64) {
    // Get current normal process
    let npt = NORMAL_PROCESS.lock();
    let mut queue = NORMAL_QUEUE.lock();

    // Get current PID
    let pid = queue[0];
    let proc = match npt.process.get(pid) {
        Some(proc) => proc,
        None => {
            crate::apic::eoi();
            return;
        }
    };

    // Check is it present
    if !proc.present {
        crate::apic::eoi();
        return;
    }

    // Save its current id and update queue
    CURRENT_ID.store(pid, Ordering::SeqCst);
    queue.remove(0);
    queue.push(pid);

    // Get info
    let cr3 = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(proc.table_addr));
    let rsp = proc.rsp;
    let rip = proc.rip;

    // Update status and switch to CR3
    unsafe {
        Cr3::write(cr3, Cr3Flags::empty());
    }

    // Finally, update RSP and jump
    crate::apic::eoi();
    unsafe {
        core::arch::asm!(
            "push 0x2b",     // SS
            "push {0}",     // RSP
            "push {1}",     // RFLAGS
            "push 0x33",    // CS, PL=3
            "push {2}",     // RIP
            "iretq",
            in(reg) rsp,
            in(reg) rflags,
            in(reg) rip,
            options(nomem, nostack, noreturn, preserves_flags)
        )
    }
}
