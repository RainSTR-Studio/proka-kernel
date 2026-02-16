//! Scheduler integration for Proka Kernel
//!
//! This module provides the integration between the thread scheduler
//! and the kernel's interrupt system.

use super::thread::{self, Context, Tid};
use spin::Mutex;

/// Global scheduler instance
pub static SCHEDULER: Mutex<thread::Scheduler> = Mutex::new(thread::Scheduler::new());

/// Flag to indicate if scheduler is initialized
static SCHEDULER_INITIALIZED: spin::RwLock<bool> = spin::RwLock::new(false);

/// Initialize the scheduler system
pub fn init() {
    let mut scheduler = SCHEDULER.lock();
    scheduler.init(thread::idle_thread);

    // Mark scheduler as initialized
    *SCHEDULER_INITIALIZED.write() = true;

    log::info!("Scheduler initialized");
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
        let mut scheduler = SCHEDULER.lock();
        scheduler
            .create_kernel_thread(entry_point, priority, Some(name))
            .map_err(|e| e.into())
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

/// Block current thread waiting for IPC
pub fn block_ipc(sender_tid: Option<Tid>, timeout_ms: Option<u64>) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler = SCHEDULER.lock();
        scheduler.block_ipc(sender_tid, timeout_ms);
    });
}

/// Unblock a thread
pub fn unblock(tid: Tid) -> Result<(), SchedulerError> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler = SCHEDULER.lock();
        scheduler.unblock(tid).map_err(|e| e.into())
    })
}

/// Get current thread ID
pub fn current_tid() -> Option<Tid> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let scheduler = SCHEDULER.lock();
        scheduler.current_tid()
    })
}

/// Get current thread name (for debugging)
pub fn current_thread_name() -> Option<alloc::string::String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let scheduler = SCHEDULER.lock();
        if let Some(tid) = scheduler.current_tid() {
            if let Some(tcb) = scheduler.get_thread(tid) {
                return tcb.name.clone();
            }
        }
        None
    })
}

/// Terminate the current thread
pub fn terminate_self() -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let tid = {
            let scheduler = SCHEDULER.lock();
            scheduler
                .current_tid()
                .expect("terminate_self called outside of thread context")
        };
        {
            let mut scheduler = SCHEDULER.lock();
            let _ = scheduler.terminate_thread(tid);
        }

        unsafe {
            schedule_next();
        }
    });

    unreachable!("Thread should have been terminated");
}

/// Timer interrupt handler for scheduling
///
/// This is called from the timer interrupt handler.
/// It performs preemptive scheduling.
pub fn timer_tick() {
    // Only schedule if scheduler is initialized
    if !is_initialized() {
        return;
    }

    unsafe {
        schedule_next();
    }
}

/// Perform a context switch to the next scheduled thread
///
/// # Safety
/// Must be called with interrupts disabled
pub unsafe fn schedule_next() {
    // Get all the information we need in one locked section
    let switch_info = {
        let mut scheduler = SCHEDULER.lock();
        let old_tid = scheduler.current_tid();
        let new_tid = scheduler.schedule();

        // If no change or no new thread, just return
        if old_tid == new_tid || new_tid.is_none() {
            None
        } else {
            let new_tid = new_tid.unwrap();

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
    };

    // Perform the context switch outside of the lock
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
}

impl From<thread::SchedulerError> for SchedulerError {
    fn from(e: thread::SchedulerError) -> Self {
        match e {
            thread::SchedulerError::MaxThreadsReached => Self::MaxThreadsReached,
            thread::SchedulerError::ThreadNotFound => Self::ThreadNotFound,
            thread::SchedulerError::NotBlocked => Self::NotBlocked,
            thread::SchedulerError::InvalidPriority => Self::InvalidPriority,
        }
    }
}
