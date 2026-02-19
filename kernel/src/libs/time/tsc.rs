use super::pit::PIT;
use core::arch::x86_64::_rdtsc;
use core::sync::atomic::{AtomicU64, Ordering};
// TSC frequency in Hz
static TSC_FREQUENCY: AtomicU64 = AtomicU64::new(0);

/// Initialize TSC by calibrating it against the PIT
pub fn init() {
    // Check if TSC is supported? (Assuming yes for x86_64)

    const CAL_MS: u64 = 50;
    const PIT_FREQ: u64 = 1_193_182;

    let (tsc_delta, pit_ticks) = super::calibrate_with_pit(CAL_MS * 1000, || {
        let start_tsc = unsafe { _rdtsc() };
        move || {
            let end_tsc = unsafe { _rdtsc() };
            end_tsc - start_tsc
        }
    });

    let freq = (tsc_delta * PIT_FREQ).checked_div(pit_ticks).unwrap_or(0);

    TSC_FREQUENCY.store(freq, Ordering::Relaxed);
}

/// Read the current TSC value
pub fn read() -> u64 {
    // Use lfence to prevent out-of-order execution if needed,
    // but for simple timing _rdtsc is often sufficient.
    // _rdtscp is better if available.
    unsafe { _rdtsc() }
}

/// Get the TSC frequency in Hz
pub fn frequency() -> u64 {
    TSC_FREQUENCY.load(Ordering::Relaxed)
}

/// Get time since boot in seconds (f64)
pub fn time_since_boot() -> f64 {
    let freq = frequency();
    if freq == 0 {
        return 0.0;
    }
    let ticks = read();
    ticks as f64 / freq as f64
}

/// Get time since boot in milliseconds
pub fn uptime_ms() -> u64 {
    let freq = frequency();
    if freq == 0 {
        return 0;
    }
    ((read() as u128 * 1000) / freq as u128) as u64
}

/// Get time since boot in microseconds
pub fn uptime_us() -> u64 {
    let freq = frequency();
    if freq == 0 {
        return 0;
    }
    ((read() as u128 * 1_000_000) / freq as u128) as u64
}

/// Sleep for a given number of microseconds using TSC
/// Requires initialization first
pub fn sleep_us(us: u64) {
    let freq = frequency();
    if freq == 0 {
        // Fallback to PIT if TSC not calibrated
        PIT.lock().sleep_blocking(us);
        return;
    }

    let ticks = (us * freq) / 1_000_000;
    let start = read();
    while read() - start < ticks {
        core::hint::spin_loop();
    }
}
