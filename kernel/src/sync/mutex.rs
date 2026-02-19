//! Mutex implementation with priority inheritance
//!
//! This mutex uses priority inheritance to prevent priority inversion.
//! When a lower-priority thread holds the lock, its priority is temporarily
//! boosted to that of the highest-priority waiting thread.

use crate::process::scheduler::{self};
use crate::process::thread::Tid;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// Priority inheritance mutex
///
/// This mutex prevents priority inversion by temporarily boosting the
/// priority of the lock holder to match the highest-priority waiter.
pub struct Mutex<T: ?Sized> {
    /// Lock state
    locked: AtomicBool,
    /// Current owner (None if unlocked)
    owner: UnsafeCell<Option<Tid>>,
    /// Original priority of the owner (for restoration)
    owner_original_priority: UnsafeCell<u8>,
    /// Queue of waiting threads (sorted by priority)
    waiters: UnsafeCell<Vec<Tid>>,
    /// Protected data
    data: UnsafeCell<T>,
}

/// Mutex guard - released when dropped
pub struct MutexGuard<'a, T: ?Sized> {
    pub(crate) mutex: &'a Mutex<T>,
}

// Safety: Mutex is Send if T is Send
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    /// Create a new unlocked mutex
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            owner: UnsafeCell::new(None),
            owner_original_priority: UnsafeCell::new(0),
            waiters: UnsafeCell::new(Vec::new()),
            data: UnsafeCell::new(data),
        }
    }

    /// Create a new locked mutex
    pub const fn new_locked(data: T) -> Self {
        Self {
            locked: AtomicBool::new(true),
            owner: UnsafeCell::new(None),
            owner_original_priority: UnsafeCell::new(0),
            waiters: UnsafeCell::new(Vec::new()),
            data: UnsafeCell::new(data),
        }
    }

    /// Consume the mutex and return the inner data
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquire the lock, blocking if necessary
    ///
    /// This function will block until the lock is acquired.
    /// Priority inheritance is applied to prevent priority inversion.
    pub fn lock(&self) -> MutexGuard<T> {
        let current_tid =
            scheduler::current_tid().expect("Mutex::lock called outside thread context");
        let current_priority = self.get_current_priority();

        loop {
            // Try to acquire the lock
            if self.try_lock_internal(current_tid, current_priority) {
                return MutexGuard { mutex: self };
            }

            // Lock is held by another thread
            // Apply priority inheritance and block
            self.apply_priority_inheritance(current_tid, current_priority);

            // Add ourselves to waiters queue
            unsafe {
                let waiters = &mut *self.waiters.get();
                if !waiters.contains(&current_tid) {
                    // Insert sorted by priority (lower number = higher priority)
                    let pos = waiters.iter().position(|&tid| {
                        self.get_thread_priority(tid)
                            .map_or(false, |p| p > current_priority)
                    });
                    match pos {
                        Some(p) => waiters.insert(p, current_tid),
                        None => waiters.push(current_tid),
                    }
                }
            }

            // Block waiting for the lock
            scheduler::block_sync(self as *const _ as u64);
            scheduler::yield_thread();

            // We've been unblocked, remove from waiters
            unsafe {
                let waiters = &mut *self.waiters.get();
                waiters.retain(|&t| t != current_tid);
            }
        }
    }

    /// Try to acquire the lock without blocking
    ///
    /// Returns Some(MutexGuard) if the lock was acquired, None otherwise.
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        let current_tid = scheduler::current_tid()?;
        let current_priority = self.get_current_priority();

        if self.try_lock_internal(current_tid, current_priority) {
            Some(MutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Check if the mutex is locked
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }

    /// Get a mutable reference to the inner data
    ///
    /// # Safety
    /// This is safe because &mut self guarantees exclusive access
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    /// Internal try lock - assumes we're in thread context
    fn try_lock_internal(&self, current_tid: Tid, _current_priority: u8) -> bool {
        // Try to acquire the lock
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // We got the lock
            unsafe {
                *self.owner.get() = Some(current_tid);
                *self.owner_original_priority.get() = _current_priority;
            }
            true
        } else {
            false
        }
    }

    /// Apply priority inheritance
    ///
    /// If the current thread has higher priority than the lock holder,
    /// boost the holder's priority.
    fn apply_priority_inheritance(&self, _waiter_tid: Tid, waiter_priority: u8) {
        unsafe {
            let owner = *self.owner.get();
            if let Some(owner_tid) = owner {
                // Get owner's current priority
                if let Some(owner_priority) = self.get_thread_priority(owner_tid) {
                    // If waiter has higher priority (lower number), boost owner
                    if waiter_priority < owner_priority {
                        // Boost owner's priority
                        let _ = scheduler::set_priority(owner_tid, waiter_priority);
                    }
                }
            }
        }
    }

    /// Restore owner's original priority when releasing lock
    fn restore_owner_priority(&self) {
        unsafe {
            if let Some(owner_tid) = *self.owner.get() {
                let original_priority = *self.owner_original_priority.get();
                let _ = scheduler::set_priority(owner_tid, original_priority);
            }
        }
    }

    /// Get current thread's priority
    fn get_current_priority(&self) -> u8 {
        scheduler::current_tid()
            .and_then(|tid| self.get_thread_priority(tid))
            .unwrap_or(128) // Default medium priority
    }

    /// Get a thread's priority from the scheduler
    fn get_thread_priority(&self, tid: Tid) -> Option<u8> {
        // Access scheduler's thread table
        // This is a bit of a hack - ideally scheduler would expose this
        x86_64::instructions::interrupts::without_interrupts(|| {
            let scheduler_opt = scheduler::SCHEDULER.lock();
            scheduler_opt
                .as_ref()?
                .get_thread(tid)
                .map(|tcb| tcb.priority)
        })
    }

    /// Release the lock and wake up a waiter
    fn unlock(&self) {
        // Restore owner's original priority
        self.restore_owner_priority();

        // Clear owner
        unsafe {
            *self.owner.get() = None;
        }

        // Release the lock
        self.locked.store(false, Ordering::Release);

        // Wake up the highest-priority waiter
        unsafe {
            let waiters = &*self.waiters.get();
            if let Some(&waiter_tid) = waiters.first() {
                let _ = scheduler::unblock(waiter_tid);
            }
        }
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

/// A simple spinlock for use in contexts where blocking is not possible
///
/// This is useful for low-level synchronization where we can't block.
pub struct SpinLock<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
}

unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<T> {
        // Spin until we acquire the lock
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }
}

impl<T: ?Sized> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Mutex tests require thread context which is not available
    // in unit tests. These tests should be run in integration tests
    // with a running scheduler.

    #[test_case]
    fn test_spinlock_basic() {
        let lock = SpinLock::new(42);
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
    }

    #[test_case]
    fn test_spinlock_mutate() {
        let lock = SpinLock::new(0);
        {
            let mut guard = lock.lock();
            *guard = 100;
        }
        {
            let guard = lock.lock();
            assert_eq!(*guard, 100);
        }
    }

    #[test_case]
    fn test_spinlock_try_lock() {
        let lock = SpinLock::new(42);

        // First lock should succeed
        let guard = lock.try_lock();
        assert!(guard.is_some());

        // Second lock should fail
        let guard2 = lock.try_lock();
        assert!(guard2.is_none());
    }
}
