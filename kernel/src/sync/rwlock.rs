//! Reader-Writer Lock for Proka Kernel
//!
//! Allows multiple readers or a single writer to hold the lock.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicIsize, Ordering};

pub struct RwLockReadGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
    interrupts_enabled: bool,
}

pub struct RwLockWriteGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
    interrupts_enabled: bool,
}

// Safety: RwLock is Send if T is Send + Sync, and Sync if T is Send + Sync
unsafe impl<T: ?Sized + Send + Sync> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

/// A reader-writer lock
pub struct RwLock<T: ?Sized> {
    /// Lock state: positive = number of readers, -1 = writer, 0 = unlocked
    /// Bit 30: writer pending flag
    state: AtomicIsize,
    /// Inner data
    data: UnsafeCell<T>,
}

const RWLOCK_WRITER: isize = -1;
const RWLOCK_WRITE_PENDING: isize = 1 << 30;

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicIsize::new(0),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Acquire read access, blocking if a writer holds the lock or is pending.
    /// Automatically disables interrupts while held.
    pub fn read(&self) -> RwLockReadGuard<T> {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        if interrupts_enabled {
            x86_64::instructions::interrupts::disable();
        }

        loop {
            let s = self.state.load(Ordering::Acquire);

            // Allow read if no writer and no writer is pending
            if s >= 0 && (s & RWLOCK_WRITE_PENDING) == 0 {
                if self
                    .state
                    .compare_exchange(s, s + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return RwLockReadGuard {
                        lock: self,
                        interrupts_enabled,
                    };
                }
                continue;
            }

            // Writer holds the lock or write is pending, block
            if interrupts_enabled {
                x86_64::instructions::interrupts::enable();
            }
            crate::process::scheduler::block_sync(self as *const _ as *const () as u64);
            crate::process::scheduler::yield_thread();
            x86_64::instructions::interrupts::disable();
        }
    }

    /// Try to acquire read access without blocking
    pub fn try_read(&self) -> Option<RwLockReadGuard<T>> {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        if interrupts_enabled {
            x86_64::instructions::interrupts::disable();
        }

        let s = self.state.load(Ordering::Acquire);
        if s >= 0 && (s & RWLOCK_WRITE_PENDING) == 0 {
            if self
                .state
                .compare_exchange(s, s + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(RwLockReadGuard {
                    lock: self,
                    interrupts_enabled,
                });
            }
        }

        if interrupts_enabled {
            x86_64::instructions::interrupts::enable();
        }
        None
    }

    /// Acquire write access, blocking if any readers or a writer hold the lock.
    /// Automatically disables interrupts while held.
    pub fn write(&self) -> RwLockWriteGuard<T> {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        if interrupts_enabled {
            x86_64::instructions::interrupts::disable();
        }

        loop {
            let s = self.state.load(Ordering::Acquire);

            // Try to grab if completely unlocked
            if s == 0 {
                if self
                    .state
                    .compare_exchange(0, RWLOCK_WRITER, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return RwLockWriteGuard {
                        lock: self,
                        interrupts_enabled,
                    };
                }
                continue;
            }

            // Lock is held, set pending flag to prevent new readers
            if (s & RWLOCK_WRITE_PENDING) == 0 {
                if self
                    .state
                    .compare_exchange(
                        s,
                        s | RWLOCK_WRITE_PENDING,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_err()
                {
                    continue;
                }
            }

            // Block
            if interrupts_enabled {
                x86_64::instructions::interrupts::enable();
            }
            crate::process::scheduler::block_sync(self as *const _ as *const () as u64);
            crate::process::scheduler::yield_thread();
            x86_64::instructions::interrupts::disable();
        }
    }

    /// Try to acquire write access without blocking
    pub fn try_write(&self) -> Option<RwLockWriteGuard<T>> {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        if interrupts_enabled {
            x86_64::instructions::interrupts::disable();
        }

        if self
            .state
            .compare_exchange(0, RWLOCK_WRITER, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            Some(RwLockWriteGuard {
                lock: self,
                interrupts_enabled,
            })
        } else {
            if interrupts_enabled {
                x86_64::instructions::interrupts::enable();
            }
            None
        }
    }

    fn unlock_read(&self) {
        let old_state = self.state.fetch_sub(1, Ordering::AcqRel);
        // If we were the last reader and a writer is pending, wake up
        if old_state == 1 || (old_state & !RWLOCK_WRITE_PENDING) == 1 {
            if (old_state & RWLOCK_WRITE_PENDING) != 0 {
                crate::process::scheduler::unblock_sync(self as *const _ as *const () as u64);
            }
        }
    }

    fn unlock_write(&self) {
        self.state.store(0, Ordering::Release);
        // Wake up all readers and writers
        crate::process::scheduler::unblock_sync(self as *const _ as *const () as u64);
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
        if self.interrupts_enabled {
            x86_64::instructions::interrupts::enable();
        }
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
        if self.interrupts_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}
