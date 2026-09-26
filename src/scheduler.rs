//! The scheduler.
extern crate alloc;
use crate::process::{Context, DRIVER_PROCESS, NORMAL_PROCESS};
use alloc::vec::Vec;
use core::{
    mem::offset_of,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

/// The normal process queue.
pub static NORMAL_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The driver process queue.
pub static DRIVER_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Assign is it driver running or normal process.
pub static IS_DRIVER: AtomicBool = AtomicBool::new(false);

// Contains the current PID/DID.
pub static CURRENT_ID: AtomicUsize = AtomicUsize::new(16383);

/// The task switcher
#[unsafe(no_mangle)]
pub extern "x86-interrupt" fn switch_task(stack: InterruptStackFrame) {
    // First, we should save the stack info before switching stack...
    let rbp_cur: u64;
    let cr3: u64;
    unsafe {
        core::arch::asm!(
            "mov {cr3}, cr3",
            "mov cr3, {krnl_pml4}", // Fixed addr, safe
            "mfence",
            "mov {rbp}, rbp",
            krnl_pml4 = in(reg) 0x100000u64,
            cr3 = out(reg) cr3,
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
        // Because of this, we need to use opposite types...
        let mut guard = NORMAL_PROCESS.write();
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
            proc.context.rbp = stack_base.offset(0).read_volatile(); // 0
            proc.context.r15 = stack_base.offset(-1).read_volatile(); // -8
            proc.context.r14 = stack_base.offset(-2).read_volatile(); // -16
            proc.context.r13 = stack_base.offset(-3).read_volatile(); // -24
            proc.context.r12 = stack_base.offset(-4).read_volatile(); // -32
            proc.context.r11 = stack_base.offset(-5).read_volatile(); // -40
            proc.context.r10 = stack_base.offset(-6).read_volatile(); // -48
            proc.context.r9 = stack_base.offset(-7).read_volatile(); // -56
            proc.context.r8 = stack_base.offset(-8).read_volatile(); // -64
            proc.context.rdi = stack_base.offset(-9).read_volatile(); // -72
            proc.context.rsi = stack_base.offset(-10).read_volatile(); // -80
            proc.context.rdx = stack_base.offset(-11).read_volatile(); // -88
            proc.context.rcx = stack_base.offset(-12).read_volatile(); // -96
            proc.context.rbx = stack_base.offset(-13).read_volatile(); // -104
            proc.context.rax = stack_base.offset(-14).read_volatile(); // -112
            proc.context.rip = rip;
            proc.context.rsp = rsp;
            proc.context.rflags = rflags;
            proc.context.cs = cs;
            proc.context.ss = ss;
            proc.current_table = cr3;
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
        let mut guard = DRIVER_PROCESS.write();
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
            proc.context.rbp = stack_base.offset(0).read_volatile(); // 0
            proc.context.r15 = stack_base.offset(-1).read_volatile(); // -8
            proc.context.r14 = stack_base.offset(-2).read_volatile(); // -16
            proc.context.r13 = stack_base.offset(-3).read_volatile(); // -24
            proc.context.r12 = stack_base.offset(-4).read_volatile(); // -32
            proc.context.r11 = stack_base.offset(-5).read_volatile(); // -40
            proc.context.r10 = stack_base.offset(-6).read_volatile(); // -48
            proc.context.r9 = stack_base.offset(-7).read_volatile(); // -56
            proc.context.r8 = stack_base.offset(-8).read_volatile(); // -64
            proc.context.rdi = stack_base.offset(-9).read_volatile(); // -72
            proc.context.rsi = stack_base.offset(-10).read_volatile(); // -80
            proc.context.rdx = stack_base.offset(-11).read_volatile(); // -88
            proc.context.rcx = stack_base.offset(-12).read_volatile(); // -96
            proc.context.rbx = stack_base.offset(-13).read_volatile(); // -104
            proc.context.rax = stack_base.offset(-14).read_volatile(); // -112
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

    unsafe {
        core::arch::asm!(
            // Push return stack
            "push qword ptr [rdi + {off_ss}]",
            "push qword ptr [rdi + {off_rsp}]",
            "push qword ptr [rdi + {off_rflags}]",
            "push qword ptr [rdi + {off_cs}]",
            "push qword ptr [rdi + {off_rip}]",
            // Switch table
            "mov cr3, rax",     // Was passed by using `in("rax") context.1`
            "mfence",
            // Restore all registers
            "mov rax, qword ptr [rdi + {off_rax}]",
            "mov rbx, qword ptr [rdi + {off_rbx}]",
            "mov rcx, qword ptr [rdi + {off_rcx}]",
            "mov rdx, qword ptr [rdi + {off_rdx}]",
            "mov rsi, qword ptr [rdi + {off_rsi}]",
            "mov rbp, qword ptr [rdi + {off_rbp}]",
            "mov r8, qword ptr [rdi + {off_r8}]",
            "mov r9, qword ptr [rdi + {off_r9}]",
            "mov r10, qword ptr [rdi + {off_r10}]",
            "mov r11, qword ptr [rdi + {off_r11}]",
            "mov r12, qword ptr [rdi + {off_r12}]",
            "mov r13, qword ptr [rdi + {off_r13}]",
            "mov r14, qword ptr [rdi + {off_r14}]",
            "mov r15, qword ptr [rdi + {off_r15}]",
            "mov rdi, qword ptr [rdi + {off_rdi}]",
            "iretq",

            off_ss = const offset_of!(Context, ss),
            off_rsp = const offset_of!(Context, rsp),
            off_rflags = const offset_of!(Context, rflags),
            off_cs = const offset_of!(Context, cs),
            off_rip = const offset_of!(Context, rip),
            off_rax = const offset_of!(Context, rax),
            off_rbx = const offset_of!(Context, rbx),
            off_rcx = const offset_of!(Context, rcx),
            off_rdx = const offset_of!(Context, rdx),
            off_rsi = const offset_of!(Context, rsi),
            off_rbp = const offset_of!(Context, rbp),
            off_r8 = const offset_of!(Context, r8),
            off_r9 = const offset_of!(Context, r9),
            off_r10 = const offset_of!(Context, r10),
            off_r11 = const offset_of!(Context, r11),
            off_r12 = const offset_of!(Context, r12),
            off_r13 = const offset_of!(Context, r13),
            off_r14 = const offset_of!(Context, r14),
            off_r15 = const offset_of!(Context, r15),
            off_rdi = const offset_of!(Context, rdi),

            in("rdi") &context.0,
            in("rax") context.1,
        );
    }
}

// Switch to next driver.
fn to_driver() -> Result<(Context, u64), ()> {
    // Get current driver process.
    let dpt = DRIVER_PROCESS.read();
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
    let npt = NORMAL_PROCESS.read();
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

    Ok((proc.context.clone(), proc.current_table))
}
