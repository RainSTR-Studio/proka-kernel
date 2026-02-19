//! Synchronization primitives for Proka Kernel
//!
//! This module provides:
//! - Mutex: Mutual exclusion with priority inheritance
//! - SpinLock: Simple spin-based locking

pub mod condvar;
pub mod mutex;
pub mod rwlock;
pub mod semaphore;

pub use condvar::Condvar;
pub use mutex::{Mutex, MutexGuard, SpinLock, SpinLockGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
pub use semaphore::Semaphore;
