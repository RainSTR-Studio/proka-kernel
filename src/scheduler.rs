//! The scheduler.
extern crate alloc;
use crate::process::{Context, DRIVER_PROCESS, NORMAL_PROCESS};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

/// The normal process queue.
pub static NORMAL_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The driver process queue.
pub static DRIVER_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Assign is it driver running or normal process.
pub static IS_DRIVER: AtomicBool = AtomicBool::new(true);

// Contains the current PID/DID.
pub static CURRENT_ID: AtomicUsize = AtomicUsize::new(16383);

/// The task switcher
pub extern "x86-interrupt" fn switch_task(stack: InterruptStackFrame) {
    // First, we should save the stack info before switching stack...
    let rbp_cur: u64;
    unsafe {
        core::arch::asm!(
            "mov rax, 0x100000", // Fixed addr, safe
            "mov cr3, rax",
            "mfence",
            "mov {rbp}, rbp",
            rbp = out(reg) rbp_cur,
        );
    }
    let stack_base = rbp_cur as *const u64;

    // Get smth bruh
    let normal_empty = NORMAL_QUEUE.lock().is_empty();
    let driver_empty = DRIVER_QUEUE.lock().is_empty();
    let rip = stack.instruction_pointer.as_u64();
    let rsp = stack.stack_pointer.as_u64();
    let cs: u64 = stack.code_segment.0.into();
    let ss: u64 = stack.stack_segment.0.into();
    let rflags = stack.cpu_flags.bits();

    // Check is queue is empty.
    if normal_empty && driver_empty {
        crate::apic::eoi();
        return;
    }

    // Before switching task, perhaps we shall save its basic info...
    // Get current id
    let current_id = CURRENT_ID.load(Ordering::Relaxed);

    // Decide run process or driver.
    let result = if IS_DRIVER.load(Ordering::Relaxed) {
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
                )
            }

            return;
        }

        // Since we entered this function, the compiler
        // has helped us to push each general registers'
        // value into stack
        //
        // Before we load it, the RBP has changed, so here
        // we just use offset to get it...
        unsafe {
            proc.context.rbp = *stack_base.offset(0); // 0
            proc.context.r15 = *stack_base.offset(-1); // -8
            proc.context.r14 = *stack_base.offset(-2); // -16
            proc.context.r13 = *stack_base.offset(-3); // -24
            proc.context.r12 = *stack_base.offset(-4); // -32
            proc.context.r11 = *stack_base.offset(-5); // -40
            proc.context.r10 = *stack_base.offset(-6); // -48
            proc.context.r9 = *stack_base.offset(-7); // -56
            proc.context.r8 = *stack_base.offset(-8); // -64
            proc.context.rdi = *stack_base.offset(-9); // -72
            proc.context.rsi = *stack_base.offset(-10); // -80
            proc.context.rdx = *stack_base.offset(-11); // -88
            proc.context.rcx = *stack_base.offset(-12); // -96
            proc.context.rbx = *stack_base.offset(-13); // -104
            proc.context.rax = *stack_base.offset(-14); // -112
            proc.context.rip = rip;
            proc.context.rsp = rsp;
            proc.context.rflags = rflags;
            proc.context.cs = cs;
            proc.context.ss = ss;
        }
        drop(guard);

        to_driver()
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
                )
            }

            return;
        }

        // Do the same here...
        unsafe {
            proc.context.rbp = *stack_base.offset(0); // 0
            proc.context.r15 = *stack_base.offset(-1); // -8
            proc.context.r14 = *stack_base.offset(-2); // -16
            proc.context.r13 = *stack_base.offset(-3); // -24
            proc.context.r12 = *stack_base.offset(-4); // -32
            proc.context.r11 = *stack_base.offset(-5); // -40
            proc.context.r10 = *stack_base.offset(-6); // -48
            proc.context.r9 = *stack_base.offset(-7); // -56
            proc.context.r8 = *stack_base.offset(-8); // -64
            proc.context.rdi = *stack_base.offset(-9); // -72
            proc.context.rsi = *stack_base.offset(-10); // -80
            proc.context.rdx = *stack_base.offset(-11); // -88
            proc.context.rcx = *stack_base.offset(-12); // -96
            proc.context.rbx = *stack_base.offset(-13); // -104
            proc.context.rax = *stack_base.offset(-14); // -112
            proc.context.rip = rip;
            proc.context.rsp = rsp;
            proc.context.rflags = rflags;
            proc.context.cs = cs;
            proc.context.ss = ss;
        }

        drop(guard);

        to_normal()
    };

    let context = if let Ok(val) = result {
        val
    } else {
        crate::apic::eoi();
        return;
    };

    // Copy all register values from tuple (Context, cr3)
    let rax = context.0.rax;
    let rcx = context.0.rcx;
    let rdx = context.0.rdx;
    let rsi = context.0.rsi;
    let rdi = context.0.rdi;
    let r8 = context.0.r8;
    let r9 = context.0.r9;
    let r10 = context.0.r10;
    let r11 = context.0.r11;
    let rbx = context.0.rbx;
    let r12 = context.0.r12;
    let r13 = context.0.r13;
    let r14 = context.0.r14;
    let r15 = context.0.r15;
    let rip = context.0.rip;
    let cs = context.0.cs;
    let rflags = context.0.rflags;
    let rsp = context.0.rsp;
    let rbp = context.0.rbp;
    let ss = context.0.ss;
    let cr3 = context.1;

    unsafe {
        // Push return stack
        core::arch::asm!(
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            ss = in(reg) ss,
            rsp = in(reg) rsp,
            rflags = in(reg) rflags,
            cs = in(reg) cs,
            rip = in(reg) rip,
        );

        // Push callee-saved group 1
        core::arch::asm!(
            "push {rbx}",
            "push {r12}",
            "push {r13}",
            "push {r14}",
            rbx = in(reg) rbx,
            r12 = in(reg) r12,
            r13 = in(reg) r13,
            r14 = in(reg) r14,
        );

        // Push callee-saved group 2
        core::arch::asm!(
            "push {r15}",
            "push {rax}",
            r15 = in(reg) r15,
            rax = in(reg) rax,
        );

        // Push scratch group 1
        core::arch::asm!(
            "push {rcx}",
            "push {rdx}",
            "push {rsi}",
            rcx = in(reg) rcx,
            rdx = in(reg) rdx,
            rsi = in(reg) rsi,
        );

        // Push scratch group 2
        core::arch::asm!(
            "push {rdi}",
            "push {r8}",
            "push {r9}",
            "push {r10}",
            rdi = in(reg) rdi,
            r8 = in(reg) r8,
            r9 = in(reg) r9,
            r10 = in(reg) r10,
        );

        // Push scratch group 3
        core::arch::asm!(
            "push {r11}",
            "push {rbp}",
            r11 = in(reg) r11,
            rbp = in(reg) rbp,
        );

        // Emit EOI
        crate::apic::eoi();

        // Pop all regs in one block
        core::arch::asm!(
            "mov rax, {cr3}",
            "mov cr3, rax",
            "mfence",
            "pop rbp",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "pop rax",
            "pop r15",
            "pop r14",
            "pop r13",
            "pop r12",
            "pop rbx",
            "iretq",
            cr3 = in(reg) cr3,
            options(noreturn)
        );
    }
}

// Switch to next driver.
fn to_driver() -> Result<(Context, u64), ()> {
    // Get current driver process.
    let dpt = DRIVER_PROCESS.lock();
    let mut queue = DRIVER_QUEUE.lock();

    // Get the process
    let did = queue[0];
    let proc = match dpt.process.get(did) {
        Some(p) => p,
        None => {
            return Err(());
        }
    };

    // Check is it present
    if !proc.present {
        return Err(());
    }

    // Save its current DID and update queue
    CURRENT_ID.store(did, Ordering::Relaxed);
    queue.remove(0);
    queue.push(did);

    Ok((proc.context.clone(), proc.table_addr))
}

// Switch to next normal process.
 
fn to_normal() -> Result<(Context, u64), ()> {
    // Get current normal process
    let npt = NORMAL_PROCESS.lock();
    let mut queue = NORMAL_QUEUE.lock();

    // Get current PID
    let pid = queue[0];
    let proc = match npt.process.get(pid) {
        Some(proc) => proc,
        None => {
            return Err(());
        }
    };

    // Check is it present
    if !proc.present {
        return Err(());
    }

    // Save its current id and update queue
    CURRENT_ID.store(pid, Ordering::Relaxed);
    queue.remove(0);
    queue.push(pid);

    Ok((proc.context.clone(), proc.table_addr))
}
