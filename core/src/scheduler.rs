//! Generic scheduler usable value.
extern crate alloc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize};
use spin::Mutex;

/// The normal process queue.
pub static NORMAL_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The driver process queue.
pub static DRIVER_QUEUE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Assign is it driver running or normal process.
pub static IS_DRIVER: AtomicBool = AtomicBool::new(false);

// Contains the current PID/DID.
pub static CURRENT_ID: AtomicUsize = AtomicUsize::new(16383);
