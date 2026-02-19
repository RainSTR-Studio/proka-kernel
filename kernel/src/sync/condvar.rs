//! Condition Variable implementation for Proka Kernel
//!
//! Condition variables allow threads to wait until a particular condition
//! becomes true. They must be used with a Mutex.

use crate::process::scheduler;
use crate::sync::mutex::MutexGuard;

/// A condition variable
pub struct Condvar {}

// Safety: Condvar is Send and Sync
unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    /// Create a new condition variable
    pub const fn new() -> Self {
        Self {}
    }

    /// Wait for a notification, releasing the mutex while waiting
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let mutex = guard.mutex;

        // Release the mutex
        drop(guard);

        // Block until notified
        scheduler::block_sync(self as *const _ as *const () as u64);
        scheduler::yield_thread();

        // Re-acquire the mutex
        mutex.lock()
    }

    /// Notify one waiting thread
    pub fn notify_one(&self) {
        // In this implementation, unblock_sync wakes all.
        // For a more precise notify_one, we'd need a different scheduler API.
        // But for microkernel IPC, waking all is a safe (if slightly less efficient) default.
        scheduler::unblock_sync(self as *const _ as *const () as u64);
    }

    /// Notify all waiting threads
    pub fn notify_all(&self) {
        scheduler::unblock_sync(self as *const _ as *const () as u64);
    }
}
