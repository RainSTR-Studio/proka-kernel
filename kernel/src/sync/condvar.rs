//! Condition Variable implementation for Proka Kernel
//!
//! Condition variables allow threads to wait until a particular condition
//! becomes true. They must be used with a Mutex.

use crate::process::scheduler;
use crate::process::thread::Tid;
use crate::sync::mutex::{Mutex, MutexGuard};
use alloc::vec::Vec;
use core::cell::UnsafeCell;

/// A condition variable
pub struct Condvar {
    /// Queue of waiting threads
    waiters: UnsafeCell<Vec<Tid>>,
}

// Safety: Condvar is Send and Sync
unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    /// Create a new condition variable
    pub const fn new() -> Self {
        Self {
            waiters: UnsafeCell::new(Vec::new()),
        }
    }

    /// Wait for a notification, releasing the mutex while waiting
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let current_tid = scheduler::current_tid().expect("Condvar::wait called outside thread context");
        let mutex = guard.mutex;

        // Add ourselves to waiters
        unsafe {
            let waiters = &mut *self.waiters.get();
            if !waiters.contains(&current_tid) {
                waiters.push(current_tid);
            }
        }

        // Release the mutex
        drop(guard);

        // Block until notified
        scheduler::block_sync(self as *const _ as u64);
        scheduler::yield_thread();

        // After unblocking, remove from waiters (if someone called notify_all but not us specifically)
        unsafe {
            let waiters = &mut *self.waiters.get();
            waiters.retain(|&t| t != current_tid);
        }

        // Re-acquire the mutex
        mutex.lock()
    }

    /// Notify one waiting thread
    pub fn notify_one(&self) {
        unsafe {
            let waiters = &mut *self.waiters.get();
            if let Some(waiter_tid) = waiters.pop() {
                let _ = scheduler::unblock(waiter_tid);
            }
        }
    }

    /// Notify all waiting threads
    pub fn notify_all(&self) {
        unsafe {
            let waiters = &mut *self.waiters.get();
            for &waiter_tid in waiters.iter() {
                let _ = scheduler::unblock(waiter_tid);
            }
            waiters.clear();
        }
    }
}
