//! Mutex implementation with priority inheritance
//!
//! This mutex uses priority inheritance to prevent priority inversion.
//! When a lower-priority thread holds the lock, its priority is temporarily
//! boosted to that of the highest-priority waiting thread.

use crate::process::scheduler::{self};
use crate::process::thread::Tid;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

/// Priority inheritance mutex
pub struct Mutex<T: ?Sized> {
    /// Lock state: bit 0 is locked, remaining bits are waiter count or status
    state: AtomicUsize,
    /// Current owner (None if unlocked)
    owner: AtomicU16, // Use 0 for None, Tid+1 for Some
    /// Original priority of the owner (for restoration)
    owner_original_priority: UnsafeCell<u8>,
    /// Protected data
    data: UnsafeCell<T>,
}

const MUTEX_LOCKED: usize = 1 << 0;
const MUTEX_HAS_WAITERS: usize = 1 << 1;

impl<T> Mutex<T> {
    /// Create a new unlocked mutex
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicUsize::new(0),
            owner: AtomicU16::new(0),
            owner_original_priority: UnsafeCell::new(0),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquire the lock, blocking if necessary.
    /// Automatically disables interrupts while held.
    pub fn lock(&self) -> MutexGuard<T> {
        // Save and disable interrupts
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        if interrupts_enabled {
            x86_64::instructions::interrupts::disable();
        }

        // Adaptive spinning
        for _ in 0..100 {
            if self
                .state
                .compare_exchange(0, MUTEX_LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                self.set_owner();
                return MutexGuard {
                    mutex: self,
                    interrupts_enabled,
                };
            }
            core::hint::spin_loop();
        }

        self.lock_slow(interrupts_enabled)
    }

    #[inline(never)]
    fn lock_slow(&self, interrupts_enabled: bool) -> MutexGuard<T> {
        let current_tid =
            scheduler::current_tid().expect("Mutex::lock called outside thread context");
        let current_priority = self.get_thread_priority(current_tid).unwrap_or(128);

        loop {
            let state = self.state.load(Ordering::Relaxed);

            // If unlocked, try to grab it
            if (state & MUTEX_LOCKED) == 0 {
                if self
                    .state
                    .compare_exchange(
                        state,
                        state | MUTEX_LOCKED,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    self.set_owner();
                    return MutexGuard {
                        mutex: self,
                        interrupts_enabled,
                    };
                }
                continue;
            }

            // Set the waiters flag
            if (state & MUTEX_HAS_WAITERS) == 0 {
                if self
                    .state
                    .compare_exchange(
                        state,
                        state | MUTEX_HAS_WAITERS,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_err()
                {
                    continue;
                }
            }

            self.apply_priority_inheritance(current_priority);

            // Re-enable interrupts while blocking to avoid system hang
            if interrupts_enabled {
                x86_64::instructions::interrupts::enable();
            }
            crate::process::scheduler::block_sync(self as *const _ as *const () as u64);
            crate::process::scheduler::yield_thread();
            x86_64::instructions::interrupts::disable();
        }
    }

    fn set_owner(&self) {
        if let Some(tid) = scheduler::current_tid() {
            self.owner.store(tid + 1, Ordering::Relaxed);
            unsafe {
                if let Some(p) = self.get_thread_priority(tid) {
                    *self.owner_original_priority.get() = p;
                }
            }
        }
    }

    fn apply_priority_inheritance(&self, waiter_priority: u8) {
        let owner_val = self.owner.load(Ordering::Relaxed);
        if owner_val > 0 {
            let owner_tid = owner_val - 1;
            if let Some(owner_priority) = self.get_thread_priority(owner_tid) {
                if waiter_priority < owner_priority {
                    let _ = scheduler::set_priority(owner_tid, waiter_priority);
                }
            }
        }
    }

    fn unlock(&self) {
        // Restore owner's original priority
        let owner_val = self.owner.load(Ordering::Relaxed);
        if owner_val > 0 {
            let owner_tid = owner_val - 1;
            unsafe {
                let original_p = *self.owner_original_priority.get();
                let _ = scheduler::set_priority(owner_tid, original_p);
            }
        }

        self.owner.store(0, Ordering::Relaxed);

        let old_state = self.state.fetch_and(!MUTEX_LOCKED, Ordering::Release);

        if (old_state & MUTEX_HAS_WAITERS) != 0 {
            self.state.store(0, Ordering::Relaxed);
            self.wake_waiters();
        }
    }

    fn wake_waiters(&self) {
        let sync_id = self as *const _ as *const () as u64;
        scheduler::unblock_sync(sync_id);
    }

    fn get_thread_priority(&self, tid: Tid) -> Option<u8> {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let sched = scheduler::SCHEDULER.lock();
            sched.as_ref()?.get_thread(tid).map(|t| t.priority)
        })
    }
}

/// Mutex guard - released when dropped
pub struct MutexGuard<'a, T: ?Sized> {
    pub(crate) mutex: &'a Mutex<T>,
    /// Saved interrupt state
    interrupts_enabled: bool,
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
        if self.interrupts_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}

// Safety: Mutex is Send if T is Send
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// A simple spinlock for use in contexts where blocking is not possible
pub struct SpinLock<T: ?Sized> {
    next_ticket: AtomicUsize,
    now_serving: AtomicUsize,
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
            next_ticket: AtomicUsize::new(0),
            now_serving: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<T> {
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        while self.now_serving.load(Ordering::Acquire) != my_ticket {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<T>> {
        let serving = self.now_serving.load(Ordering::Relaxed);
        if self
            .next_ticket
            .compare_exchange(serving, serving + 1, Ordering::Acquire, Ordering::Relaxed)
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
        self.lock.now_serving.fetch_add(1, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let guard = lock.try_lock();
        assert!(guard.is_some());
        let guard2 = lock.try_lock();
        assert!(guard2.is_none());
    }
}
