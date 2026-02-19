//! Semaphore implementation for Proka Kernel
//!
//! Semaphores are synchronization primitives that allow a fixed number
//! of threads to access a resource simultaneously.

use crate::process::scheduler;
use core::sync::atomic::{AtomicUsize, Ordering};

/// A counting semaphore
pub struct Semaphore {
    /// Current count
    count: AtomicUsize,
}

// Safety: Semaphore is Send and Sync
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Semaphore {
    /// Create a new semaphore with an initial count
    pub const fn new(initial_count: usize) -> Self {
        Self {
            count: AtomicUsize::new(initial_count),
        }
    }

    /// Acquire a permit from the semaphore, blocking if necessary
    pub fn wait(&self) {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current > 0 {
                if self
                    .count
                    .compare_exchange(current, current - 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return;
                }
                continue;
            }

            // No permits available, block
            scheduler::block_sync(self as *const _ as *const () as u64);
            scheduler::yield_thread();
        }
    }

    /// Try to acquire a permit without blocking
    pub fn try_wait(&self) -> bool {
        let current = self.count.load(Ordering::Acquire);
        if current > 0 {
            self.count
                .compare_exchange(current, current - 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        } else {
            false
        }
    }

    /// Release a permit back to the semaphore
    pub fn signal(&self) {
        self.count.fetch_add(1, Ordering::Release);

        // Wake up a waiter if any
        scheduler::unblock_sync(self as *const _ as *const () as u64);
    }

    /// Get current count (for debugging)
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
}
