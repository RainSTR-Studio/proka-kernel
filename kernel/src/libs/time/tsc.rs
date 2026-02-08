use super::pit::PIT;
use core::arch::x86_64::_rdtsc;
use core::sync::atomic::{AtomicU64, Ordering};
// TSC frequency in Hz
static TSC_FREQUENCY: AtomicU64 = AtomicU64::new(0);

/// Initialize TSC by calibrating it against the PIT
pub fn init() {
    let mut pit = PIT.lock();

    // Check if TSC is supported? (Assuming yes for x86_64)

    // We will wait for approximately 50ms worth of PIT ticks
    // 1193182 Hz / 1000 * 50 ~= 59659 ticks
    // This fits in u16 (65535)
    const PIT_FREQ: u64 = 1_193_182;
    const CAL_MS: u64 = 50;
    const TARGET_TICKS: u16 = (PIT_FREQ * CAL_MS / 1000) as u16;

    // Run calibration with interrupts disabled to minimize jitter
    let freq = x86_64::instructions::interrupts::without_interrupts(|| {
        // Setup PIT Ch2 in Mode 0 with max count
        // We start from 0xFFFF (65535) and count down
        pit.start_one_shot(0xFFFF);

        // Wait for a short while to ensure counting started and stable
        // (Just reading it once is usually enough to settle?)
        let start_pit = pit.read_count();
        let start_tsc = unsafe { _rdtsc() };

        let mut end_pit;
        let mut end_tsc;

        loop {
            end_pit = pit.read_count();
            end_tsc = unsafe { _rdtsc() };

            // Check delta (handling potential wrap if any, though Mode 0 shouldn't)
            // Since we count down: delta = start - end
            if start_pit >= end_pit {
                let delta = start_pit - end_pit;
                if delta >= TARGET_TICKS {
                    break;
                }
            } else {
                // If end > start, something weird happened (reload? or wrap?)
                // Just break to avoid infinite loop
                break;
            }
        }

        let pit_delta = (start_pit - end_pit) as u64;
        let tsc_delta = end_tsc - start_tsc;

        // Calculate Frequency
        // freq = tsc_delta / time
        // time = pit_delta / PIT_FREQ
        // freq = tsc_delta * PIT_FREQ / pit_delta

        (tsc_delta * PIT_FREQ).checked_div(pit_delta).unwrap_or(0)
    });

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
