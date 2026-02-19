//! Scheduler integration for Proka Kernel
//!
//! This module provides the integration between the thread scheduler
//! and the kernel's interrupt system.

use super::process;
use super::process::Pid;
use super::schedulers::PriorityScheduler;
use super::schedulers::RoundRobinScheduler;
use super::thread::{Context, ThreadControlBlock, Tid};
use alloc::boxed::Box;
use spin::Mutex;
use x86_64::PhysAddr;

/// Global scheduler instance
/// Note: This is pub(crate) to allow access from sync module while
/// maintaining encapsulation. External code should use the public API.
pub(crate) static SCHEDULER: Mutex<Option<Box<dyn Scheduler>>> = Mutex::new(None);

/// Flag to indicate if scheduler is initialized
static SCHEDULER_INITIALIZED: spin::RwLock<bool> = spin::RwLock::new(false);

/// Scheduler trait to allow multiple implementations
pub trait Scheduler: Send {
    /// Initialize the scheduler with an idle thread
    fn init(&mut self, idle_entry: extern "C" fn() -> !);
    /// Get the next thread to run
    fn schedule(&mut self) -> Option<Tid>;
    /// Create a new kernel thread
    fn create_kernel_thread(
        &mut self,
        entry_point: extern "C" fn() -> !,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError>;
    /// Create a new user thread within a given process
    fn create_user_thread(
        &mut self,
        pid: Pid,
        entry_point: usize,
        user_stack_top: usize,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError>;
    /// Terminate a thread
    fn terminate_thread(&mut self, tid: Tid) -> Result<(), SchedulerError>;
    /// Block current thread waiting for IPC
    fn block_ipc(&mut self, sender_tid: Option<Tid>, timeout_ms: Option<u64>);
    /// Block current thread to sleep until a certain uptime
    fn block_sleep(&mut self, until_ms: u64);
    /// Block current thread waiting for a child process
    fn block_wait(&mut self, target_pid: Option<Pid>);
    /// Block current thread waiting for another thread to exit
    fn block_join(&mut self, target_tid: Tid);
    /// Block current thread waiting for synchronization
    fn block_sync(&mut self, sync_id: u64);
    /// Unblock a thread (e.g., when IPC message arrives)
    fn unblock(&mut self, tid: Tid) -> Result<(), SchedulerError>;
    /// Get current running thread's TID
    fn current_tid(&self) -> Option<Tid>;
    /// Get reference to a thread
    fn get_thread(&self, tid: Tid) -> Option<&ThreadControlBlock>;
    /// Get mutable reference to a thread
    fn get_thread_mut(&mut self, tid: Tid) -> Option<&mut ThreadControlBlock>;
    /// Change the priority of a thread
    fn set_priority(&mut self, tid: Tid, new_priority: u8) -> Result<(), SchedulerError>;
    /// Yield the current thread
    fn yield_current(&mut self);
    /// Reap zombie threads and free their resources
    fn reap_zombies(&mut self);
    /// Check for sleeping threads that should be woken up
    fn wake_sleeping_threads(&mut self, current_uptime_ms: u64);
}

/// Initialize the scheduler system
pub fn init() {
    {
        let mut scheduler_opt = SCHEDULER.lock();

        // Select scheduler based on Kconfig
        let scheduler: Box<dyn Scheduler> = match crate::config::SCHEDULER_TYPE {
            "Priority" => Box::new(PriorityScheduler::new()),
            "RoundRobin" => Box::new(RoundRobinScheduler::new()),
            _ => Box::new(RoundRobinScheduler::new()), // Default to RoundRobin
        };

        *scheduler_opt = Some(scheduler);
        let scheduler = scheduler_opt.as_mut().unwrap();
        scheduler.init(idle_thread);
    }

    // Mark scheduler as initialized
    *SCHEDULER_INITIALIZED.write() = true;

    log::info!("Scheduler initialized: {}", crate::config::SCHEDULER_TYPE);
}

/// Set a custom scheduler
pub fn set_scheduler(new_scheduler: Box<dyn Scheduler>) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        *scheduler_opt = Some(new_scheduler);
    });
}

/// Check if scheduler is initialized
pub fn is_initialized() -> bool {
    *SCHEDULER_INITIALIZED.read()
}

/// Create a new kernel thread
pub fn create_kernel_thread(
    entry_point: extern "C" fn() -> !,
    priority: u8,
    name: &str,
) -> Result<Tid, SchedulerError> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        let scheduler = scheduler_opt
            .as_mut()
            .ok_or(SchedulerError::NotInitialized)?;
        scheduler.create_kernel_thread(entry_point, priority, Some(name))
    })
}

/// Yield the current thread
///
/// This function triggers a reschedule.
pub fn yield_thread() {
    // Only schedule if scheduler is initialized
    if !is_initialized() {
        return;
    }

    unsafe {
        // Disable interrupts during context switch
        x86_64::instructions::interrupts::without_interrupts(|| {
            schedule_next();
        });
    }
}

/// Block current thread for a duration in milliseconds
pub fn thread_sleep(ms: u64) {
    let until = crate::libs::time::uptime_ms() + ms;
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            scheduler.block_sleep(until);
        }
    });
    // Trigger reschedule
    yield_thread();
}

/// Block current thread until a child process exits
pub fn wait_child(target_pid: Option<Pid>) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            scheduler.block_wait(target_pid);
        }
    });
    // Trigger reschedule
    yield_thread();
}

/// Block current thread until another thread exits
pub fn thread_join(target_tid: Tid) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            // Check if thread still exists
            if let Some(tcb) = scheduler.get_thread(target_tid) {
                if tcb.state != ThreadState::Terminated {
                    scheduler.block_join(target_tid);
                }
            }
        }
    });
    // Trigger reschedule
    yield_thread();
}

/// Unblock a thread
pub fn unblock(tid: Tid) -> Result<(), SchedulerError> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        let scheduler = scheduler_opt
            .as_mut()
            .ok_or(SchedulerError::NotInitialized)?;
        scheduler.unblock(tid)
    })
}

/// Get current thread ID
pub fn current_tid() -> Option<Tid> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let scheduler_opt = SCHEDULER.lock();
        scheduler_opt.as_ref().and_then(|s| s.current_tid())
    })
}

/// Get current thread name (for debugging)
pub fn current_thread_name() -> Option<alloc::string::String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_ref() {
            if let Some(tid) = scheduler.current_tid() {
                if let Some(tcb) = scheduler.get_thread(tid) {
                    return tcb.name.clone();
                }
            }
        }
        None
    })
}

/// Terminate the current thread
pub fn terminate_self() -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let tid = {
            let scheduler_opt = SCHEDULER.lock();
            let scheduler = scheduler_opt
                .as_ref()
                .expect("terminate_self called before scheduler init");
            scheduler
                .current_tid()
                .expect("terminate_self called outside of thread context")
        };
        {
            let mut scheduler_opt = SCHEDULER.lock();
            let scheduler = scheduler_opt.as_mut().unwrap();
            let _ = scheduler.terminate_thread(tid);
        }

        unsafe {
            schedule_next();
        }
    });

    unreachable!("Thread should have been terminated");
}

/// Set the priority of a thread
pub fn set_priority(tid: Tid, priority: u8) -> Result<(), SchedulerError> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        let scheduler = scheduler_opt
            .as_mut()
            .ok_or(SchedulerError::NotInitialized)?;
        scheduler.set_priority(tid, priority)
    })
}

/// Set the priority of the current thread
pub fn set_current_priority(priority: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            if let Some(tid) = scheduler.current_tid() {
                let _ = scheduler.set_priority(tid, priority);
            }
        }
    });
}

/// Timer interrupt handler for scheduling
///
/// This is called from the timer interrupt handler.
/// It performs preemptive scheduling with time slice checking.
pub fn timer_tick() {
    // Only schedule if scheduler is initialized
    if !is_initialized() {
        return;
    }

    // Check for sleeping threads to wake
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            scheduler.wake_sleeping_threads(crate::libs::time::uptime_ms());
        }
    });

    // Check if current thread's time slice is exhausted
    let should_switch = x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            if let Some(current_tid) = scheduler.current_tid() {
                if let Some(tcb) = scheduler.get_thread_mut(current_tid) {
                    // Decrement time slice and check if exhausted
                    if tcb.tick_time_slice() {
                        // Time slice exhausted, need to switch
                        tcb.reset_time_slice();
                        return true;
                    }
                }
            }
        }
        false
    });

    if should_switch {
        unsafe {
            schedule_next();
        }
    }
}

/// Perform a context switch to the next scheduled thread
///
/// # Safety
/// Must be called with interrupts disabled
pub unsafe fn schedule_next() {
    // 1. Reap zombies before scheduling to keep memory pressure low
    {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            scheduler.reap_zombies();
        }
    }

    // 2. Get all the information we need in one locked section
    let switch_info = {
        let mut scheduler_opt = SCHEDULER.lock();
        if let Some(scheduler) = scheduler_opt.as_mut() {
            let old_tid = scheduler.current_tid();
            let new_tid = scheduler.schedule();

            // If no change or no new thread, just return None from this block
            if old_tid == new_tid || new_tid.is_none() {
                None
            } else {
                let new_tid = new_tid.unwrap();

                // Get PIDs to check if we need to switch address space
                let old_pid = old_tid.and_then(|tid| scheduler.get_thread(tid).map(|t| t.pid));
                let new_pid = scheduler.get_thread(new_tid).map(|t| t.pid);

                if old_pid != new_pid {
                    // Switch address space
                    if let Some(new_pcb_arc) = process::lock().get_process(new_pid.unwrap()) {
                        let new_pcb = new_pcb_arc.lock();
                        let pml4_phys = new_pcb.vspace;
                        let pml4_frame =
                            x86_64::structures::paging::PhysFrame::from_start_address_unchecked(
                                pml4_phys,
                            );
                        unsafe {
                            x86_64::registers::control::Cr3::write(
                                pml4_frame,
                                x86_64::registers::control::Cr3Flags::empty(),
                            );
                        }
                    }
                }

                // Get context pointers
                let old_ctx = old_tid.and_then(|tid| {
                    scheduler
                        .get_thread(tid)
                        .map(|tcb| &tcb.context as *const Context as *mut Context)
                });

                let new_ctx = scheduler
                    .get_thread(new_tid)
                    .map(|tcb| &tcb.context as *const Context);

                Some((old_ctx, new_ctx, old_tid.is_none()))
            }
        } else {
            None
        }
    };

    // 3. Perform the context switch outside of the lock
    if let Some((old_ctx, new_ctx, is_first)) = switch_info {
        match (old_ctx, new_ctx) {
            (Some(old_ctx), Some(new_ctx)) => {
                // Switch to the new thread
                super::context_switch::switch_context(old_ctx, new_ctx);
            }
            (None, Some(new_ctx)) if is_first => {
                // First context switch (from boot to first thread)
                super::context_switch::first_context_switch(new_ctx);
            }
            _ => {
                crate::serial_println!("[Scheduler] Error: Invalid context pointers!");
            }
        }
    }
}

/// Scheduler errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    MaxThreadsReached,
    ThreadNotFound,
    NotBlocked,
    InvalidPriority,
    NotInitialized,
    PidNotFound,
}

/// Allocate a kernel stack
///
/// Returns (top_addr, phys_base, page_count)
pub fn allocate_kernel_stack(size: usize) -> (usize, PhysAddr, usize) {
    use crate::memory::frame::FRAME_ALLOCATOR;
    use crate::memory::paging::phys_to_virt;

    let pages = size.div_ceil(4096);

    let frame = FRAME_ALLOCATOR
        .allocate_contiguous(pages)
        .expect("Failed to allocate kernel stack");

    // Get virtual address via HHDM
    let phys_addr = frame.start_address();
    let virt_addr = phys_to_virt(phys_addr);

    // Stack grows down, so return top address (virtual)
    let top = virt_addr.as_u64() as usize + (pages * 4096);
    (top, phys_addr, pages)
}

/// Idle thread - runs when no other threads are ready
pub extern "C" fn idle_thread() -> ! {
    loop {
        // Halt CPU until next interrupt
        unsafe { core::arch::asm!("hlt") };
    }
}
