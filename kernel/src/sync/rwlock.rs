//! Reader-Writer Lock for Proka Kernel
//!
//! Allows multiple readers or a single writer to hold the lock.

use crate::process::scheduler;
use crate::process::thread::Tid;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicIsize, Ordering};

/// A reader-writer lock
pub struct RwLock<T: ?Sized> {
    /// Lock state: positive = number of readers, -1 = writer, 0 = unlocked
    state: AtomicIsize,
    /// Queue of threads waiting for read access
    read_waiters: UnsafeCell<Vec<Tid>>,
    /// Queue of threads waiting for write access
    write_waiters: UnsafeCell<Vec<Tid>>,
    /// Inner data
    data: UnsafeCell<T>,
}

pub struct RwLockReadGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

pub struct RwLockWriteGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

// Safety: RwLock is Send if T is Send + Sync, and Sync if T is Send + Sync
unsafe impl<T: ?Sized + Send + Sync> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicIsize::new(0),
            read_waiters: UnsafeCell::new(Vec::new()),
            write_waiters: UnsafeCell::new(Vec::new()),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Acquire read access, blocking if a writer holds the lock
    pub fn read(&self) -> RwLockReadGuard<T> {
        let current_tid = scheduler::current_tid().expect("RwLock::read called outside thread context");

        loop {
            let s = self.state.load(Ordering::Acquire);
            if s >= 0 {
                // No writer, try to increment reader count
                if self.state.compare_exchange(s, s + 1, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                    return RwLockReadGuard { lock: self };
                }
                continue;
            }

            // Writer holds the lock or write is pending, block
            unsafe {
                let waiters = &mut *self.read_waiters.get();
                if !waiters.contains(&current_tid) {
                    waiters.push(current_tid);
                }
            }

            scheduler::block_sync(self as *const _ as u64);
            scheduler::yield_thread();

            unsafe {
                let waiters = &mut *self.read_waiters.get();
                waiters.retain(|&t| t != current_tid);
            }
        }
    }

    /// Try to acquire read access without blocking
    pub fn try_read(&self) -> Option<RwLockReadGuard<T>> {
        let s = self.state.load(Ordering::Acquire);
        if s >= 0 {
            if self
                .state
                .compare_exchange(s, s + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(RwLockReadGuard { lock: self });
            }
        }
        None
    }

    /// Acquire write access, blocking if any readers or a writer hold the lock
    pub fn write(&self) -> RwLockWriteGuard<T> {
        let current_tid = scheduler::current_tid().expect("RwLock::write called outside thread context");

        loop {
            // Try to set state to -1 if it's currently 0
            if self.state.compare_exchange(0, -1, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return RwLockWriteGuard { lock: self };
            }

            // Lock is held, block
            unsafe {
                let waiters = &mut *self.write_waiters.get();
                if !waiters.contains(&current_tid) {
                    waiters.push(current_tid);
                }
            }

            scheduler::block_sync(self as *const _ as u64);
            scheduler::yield_thread();

            unsafe {
                let waiters = &mut *self.write_waiters.get();
                waiters.retain(|&t| t != current_tid);
            }
        }
    }

    /// Try to acquire write access without blocking
    pub fn try_write(&self) -> Option<RwLockWriteGuard<T>> {
        if self
            .state
            .compare_exchange(0, -1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            Some(RwLockWriteGuard { lock: self })
        } else {
            None
        }
    }

    fn unlock_read(&self) {
        let s = self.state.fetch_sub(1, Ordering::AcqRel);
        if s == 1 {
            // Last reader finished, wake up a writer
            unsafe {
                let waiters = &*self.write_waiters.get();
                if let Some(&waiter_tid) = waiters.first() {
                    let _ = scheduler::unblock(waiter_tid);
                }
            }
        }
    }

    fn unlock_write(&self) {
        self.state.store(0, Ordering::Release);

        // Wake up all readers or one writer
        // Priority: wake readers first to maximize concurrency
        unsafe {
            let read_waiters = &*self.read_waiters.get();
            if !read_waiters.is_empty() {
                for &tid in read_waiters {
                    let _ = scheduler::unblock(tid);
                }
            } else {
                let write_waiters = &*self.write_waiters.get();
                if let Some(&tid) = write_waiters.first() {
                    let _ = scheduler::unblock(tid);
                }
            }
        }
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.unlock_read();
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.unlock_write();
    }
}
