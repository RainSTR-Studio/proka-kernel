//! The scheduler.
extern crate alloc;
use crate::{
    process::{DRIVER_PROCESS, NORMAL_PROCESS},
    serial_println,
    tables::gdt::GDT,
};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

/// The normal process queue.
pub static NORMAL_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The driver process queue.
pub static DRIVER_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Assign is it driver running or normal process.
static IS_DRIVER: AtomicBool = AtomicBool::new(true);

// Contains the current PID/DID.
static CURRENT_ID: AtomicUsize = AtomicUsize::new(16383);

/// The task switcher
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn switch_task(stack: InterruptStackFrame) {
    // First, we should save the stack info before switching stack...
    const TARGET_ADDR: u64 = 0xFFFF8000801FF000; // Mapped, writable
    unsafe {
        const SIZE: usize = core::mem::size_of::<InterruptStackFrame>();

        // Copy and switch to kernel page table then
        core::arch::asm!(
            "mov rsi, {src}",
            "mov rdi, {dst}",
            "mov rcx, {count:r}",
            "cld",
            "rep movsb",
            "mov rax, 0x100000", // Fixed addr, safe
            "mov cr3, rax",
            src = in(reg) &stack as *const InterruptStackFrame as usize,
            dst = in(reg) TARGET_ADDR,
            count = in(reg) SIZE,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Update to new stack frame
    // SAFETY: Target already mapped and usable
    let stack_frame = unsafe { &*(TARGET_ADDR as *const InterruptStackFrame) };
    serial_println!("\x1b[34m[DEBUG] Stack frame: {:?}\x1b[0m", stack_frame);

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
        // Now we are running driver.
        // Check: Is next proc area queue empty
        if !normal_empty {
            IS_DRIVER.store(false, Ordering::Relaxed);
        }

        // So let's save its RIP and RSP
        let mut guard = DRIVER_PROCESS.lock();
        let proc = &mut guard.process[current_id];

        // Check: Is driver empty
        if driver_empty {
            // The current ID is not changed, so we can
            // still use it to switch back...
            let cr3 = proc.table_addr;
            drop(guard);
            crate::apic::eoi();

            // Check: is cr3 empty
            if cr3 == 0 {
                return;
            }

            // Switch table
            unsafe {
                core::arch::asm!(
                    "mov rax, {0}",
                    "mov cr3, rax",
                    in(reg) cr3,
                    options(nomem, nostack, preserves_flags)
                )
            }

            return;
        }

        proc.context.rip = rip;
        proc.context.rsp = rsp;
        drop(guard);

        to_driver(rflags)
    } else {
        // Now we are running normal process.
        // Check: Is next proc area queue empty
        if !driver_empty {
            IS_DRIVER.store(true, Ordering::Relaxed);
        }

        // Do the save step as above
        let mut guard = NORMAL_PROCESS.lock();
        let proc = &mut guard.process[current_id];

        // Check: Is normal list empty
        if normal_empty {
            // Current ID still usable, because it wasn't changed
            let cr3 = proc.table_addr;
            drop(guard);
            crate::apic::eoi();

            // Check: is cr3 empty
            if cr3 == 0 {
                return;
            }

            // Switch table
            unsafe {
                core::arch::asm!(
                    "mov rax, {0}",
                    "mov cr3, rax",
                    in(reg) cr3,
                    options(nomem, nostack, preserves_flags)
                )
            }

            return;
        }

        proc.context.rip = rip;
        proc.context.rsp = rsp;
        drop(guard);

        to_normal(rflags)
    };
}

// Switch to next driver.
#[unsafe(link_section = ".gdata")]
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
    CURRENT_ID.store(did, Ordering::Relaxed);
    queue.remove(0);
    queue.push(did);

    // Now get its information
    let cr3 = proc.table_addr;
    let rsp = proc.context.rsp;
    let rip = proc.context.rip;

    // Drop locks
    drop(dpt);
    drop(queue);

    // Update status and switch to target process's table
    // proc.status = Status::Running;
    // Update RSP and jump
    crate::apic::eoi();
    let sel = GDT.1;
    unsafe {
        core::arch::asm!(
            "mov rax, {0}",
            "mov cr3, rax",
            "push {1:r}",   // SS
            "push {2}",     // RSP
            "push {3}",     // RFLAGS
            "push {4:r}",   // CS, PL=0
            "push {5}",     // RIP
            "iretq",
            in(reg) cr3,
            in(reg) sel.kernel_data.0,
            in(reg) rsp,
            in(reg) rflags,
            in(reg) sel.kernel_code.0,
            in(reg) rip,
            options(nomem, nostack, noreturn, preserves_flags)
        )
    }
}

#[unsafe(link_section = ".gdata")]
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
    CURRENT_ID.store(pid, Ordering::Relaxed);
    queue.remove(0);
    queue.push(pid);

    // Get info
    let cr3 = proc.table_addr;
    let rsp = proc.context.rsp;
    let rip = proc.context.rip;

    // Drop locks
    drop(npt);
    drop(queue);

    // Update status and switch to CR3
    // Finally, update RSP and jump
    crate::apic::eoi();
    let sel = GDT.1;
    unsafe {
        core::arch::asm!(
            "mov rax, {0}",
            "mov cr3, rax",
            "push {1:r}",     // SS
            "push {2}",       // RSP
            "push {3}",       // RFLAGS
            "push {4:r}",     // CS, PL=3
            "push {5}",       // RIP
            "iretq",
            in(reg) cr3,
            in(reg) sel.user_data.0,
            in(reg) rsp,
            in(reg) rflags,
            in(reg) sel.user_code.0,
            in(reg) rip,
            options(nomem, nostack, noreturn, preserves_flags)
        )
    }
}
