pub mod apic;
pub mod pit;
pub mod rtc;
pub mod tsc;

pub use tsc::{init, sleep_us, time_since_boot};

/// Calibrate a timer using the PIT (Programmable Interval Timer).
///
/// # Arguments
/// * `us` - The duration to wait for in microseconds.
/// * `f` - A closure that starts the timer and returns another closure to stop it.
///
/// # Returns
/// A tuple containing (measured_value, actual_pit_ticks).
pub fn calibrate_with_pit<F, R>(us: u64, f: F) -> (u64, u64)
where
    F: FnOnce() -> R,
    R: FnOnce() -> u64,
{
    let mut pit = pit::PIT.lock();
    // PIT freq is 1.193182 MHz
    let ticks = (us * 1_193_182) / 1_000_000;

    x86_64::instructions::interrupts::without_interrupts(|| {
        pit.start_one_shot(ticks as u16);
        let stop_fn = f();

        // Wait for PIT to finish (Port 0x61, Bit 5 goes HIGH)
        unsafe {
            while (x86_64::instructions::port::Port::<u8>::new(0x61).read() & 0x20) == 0 {
                core::hint::spin_loop();
            }
        }

        (stop_fn(), ticks as u64)
    })
}
