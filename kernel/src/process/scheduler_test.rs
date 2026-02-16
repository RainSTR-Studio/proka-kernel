//! Scheduler tests for Proka Kernel

use super::scheduler;
use core::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
static THREAD_A_COUNT: AtomicU64 = AtomicU64::new(0);
static THREAD_B_COUNT: AtomicU64 = AtomicU64::new(0);

/// Test function for thread A
extern "C" fn thread_a_entry() -> ! {
    use crate::println;
    println!("[Thread A] Started");
    for i in 0..10 {
        THREAD_A_COUNT.fetch_add(1, Ordering::Relaxed);
        if i % 2 == 0 {
            println!("[Thread A] Count: {}", i);
            scheduler::yield_thread();
        }
    }

    println!("[Thread A] Finished");
    // Mark test as complete
    TEST_COUNTER.fetch_add(1, Ordering::Relaxed);

    // Exit thread
    scheduler::terminate_self();
}

/// Test function for thread B
extern "C" fn thread_b_entry() -> ! {
    use crate::println;
    println!("[Thread B] Started");
    for i in 0..10 {
        THREAD_B_COUNT.fetch_add(1, Ordering::Relaxed);
        if i % 2 == 0 {
            println!("[Thread B] Count: {}", i);
            scheduler::yield_thread();
        }
    }

    println!("[Thread B] Finished");
    // Mark test as complete
    TEST_COUNTER.fetch_add(1, Ordering::Relaxed);

    // Exit thread
    scheduler::terminate_self();
}

/// Run scheduler tests
pub fn run_tests() {
    use crate::println;

    println!("[Scheduler Test] Starting...");

    // Check current thread ID
    if let Some(tid) = scheduler::current_tid() {
        println!("[Scheduler Test] Current TID: {}", tid);
    } else {
        println!("[Scheduler Test] No current thread");
    }

    // Create test threads
    println!("[Scheduler Test] Creating thread A...");
    match scheduler::create_kernel_thread(thread_a_entry, 5, "thread_a") {
        Ok(tid_a) => {
            println!("[Scheduler Test] Thread A created OK, tid={}", tid_a);

            println!("[Scheduler Test] Creating thread B...");
            match scheduler::create_kernel_thread(thread_b_entry, 5, "thread_b") {
                Ok(tid_b) => {
                    println!("[Scheduler Test] Thread B created OK, tid={}", tid_b);

                    // Try context switch
                    println!("[Scheduler Test] Testing yield...");
                    scheduler::yield_thread();
                    println!("[Scheduler Test] Yield returned!");
                }
                Err(_e) => {
                    println!("[Scheduler Test] Thread B creation failed");
                }
            }
        }
        Err(_e) => {
            println!("[Scheduler Test] Thread A creation failed");
        }
    }
}
