//! The scheduler.
extern crate alloc;
use alloc::vec::Vec;
use x86_64::{PhysAddr, registers::control::{Cr3, Cr3Flags}, structures::paging::{PhysFrame, Size4KiB}};
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::process::DRIVER_PROCESS;

/// The normal process queue.
pub static NORMAL_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The driver process queue.
pub static DRIVER_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Assign is it driver running or normal process.
static IS_DRIVER: AtomicBool = AtomicBool::new(true);

/// The task switcher
pub fn switch_task() {
    let normal_empty = NORMAL_QUEUE.lock().is_empty();
    let driver_empty = DRIVER_QUEUE.lock().is_empty();

    // Check is queue is empty.
    if normal_empty && driver_empty {
        return;
    }

    // Decide run process or driver.
    if IS_DRIVER.load(Ordering::Relaxed) {
        IS_DRIVER.store(false, Ordering::SeqCst);
        to_driver()
    } else {
        IS_DRIVER.store(true, Ordering::SeqCst);
        to_normal()
    };
}

// Switch to next driver.
fn to_driver() {
    // Get current driver process.
    let dpt = DRIVER_PROCESS.lock();
    let queue = DRIVER_QUEUE.lock();
    if queue.is_empty() {
        return;
    }

    // Get the process
    let did = queue[0];
    let proc = match dpt.process.get(did) {
        Some(p) => p,
        None => return,
    };

    // Check is it present
    if !proc.present { return; }

    // Now get its information
    let cr3 = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(proc.table_addr));
    let rsp = proc.rsp;
    let rip = proc.rip;

    // Update status and switch to target process's table
    // proc.status = Status::Running;
    unsafe { Cr3::write(cr3, Cr3Flags::empty()); }

    // Update RSP and jump
    unsafe {
        core::arch::asm!(
            "mov rsp, {0}",
            "mov rbp, rsp",
            "jmp {1}",
            in(reg) rsp,
            in(reg) rip,
            options(nomem, nostack)
        )
    }
}

fn to_normal() {}
