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
    loop {
        THREAD_A_COUNT.fetch_add(1, Ordering::Relaxed);
        let i = THREAD_A_COUNT.load(Ordering::Relaxed);
        if i % 50 == 0 {
            println!("[Thread A] Iteration {}", i);
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
    loop {
        THREAD_B_COUNT.fetch_add(1, Ordering::Relaxed);
        let i = THREAD_B_COUNT.load(Ordering::Relaxed);
        if i % 50 == 0 {
            println!("[Thread B] Iteration {}", i);
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

    // Create test threads
    println!("[Scheduler Test] Creating thread A...");
    let tid_a = scheduler::create_kernel_thread(thread_a_entry, 5, "thread_a").unwrap();
    println!("[Scheduler Test] Thread A created OK, tid={}", tid_a);

    println!("[Scheduler Test] Creating thread B...");
    let tid_b = scheduler::create_kernel_thread(thread_b_entry, 5, "thread_b").unwrap();
    println!("[Scheduler Test] Thread B created OK, tid={}", tid_b);

    // Now both threads are in the ready queue.
    // Set main thread priority to same as A/B (5) to test interleaving
    scheduler::set_current_priority(5);

    // Yield main thread to let them run
    println!("[Scheduler Test] Testing multi-thread interleaving...");
    scheduler::yield_thread();
    println!("[Scheduler Test] Yield returned!");

    // Set priority back to high to finish test/shell setup
    scheduler::set_current_priority(0);
}
