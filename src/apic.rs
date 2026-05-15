//! The APIC module.
use core::sync::atomic::AtomicU32;
use spin::{Lazy, Mutex};
use x2apic::lapic::{LocalApic as LocalApicOut, LocalApicBuilder, TimerDivide, TimerMode};
use x86_64::instructions::port::Port;

// Constants
pub const XAPIC_BASE: u64 = 0xFFFFE08000000000;

// Global statics
/// The local APIC.
pub static LAPIC: Lazy<Mutex<LocalApic>> = Lazy::new(|| {
    let mut lapic_cfg = LocalApicBuilder::new();

    // Set XAPIC base (mapped)
    lapic_cfg.set_xapic_base(XAPIC_BASE);

    // Set up timer
    lapic_cfg.timer_divide(TimerDivide::Div16);
    lapic_cfg.timer_initial(10000);
    lapic_cfg.timer_mode(TimerMode::Periodic);
    lapic_cfg.timer_vector(0x30);

    // Set up sprious and error handler
    lapic_cfg.spurious_vector(0xF0);
    lapic_cfg.error_vector(0xF1);

    // Build and store
    let lapic = lapic_cfg.build().unwrap();
    Mutex::new(LocalApic(lapic))
});

pub static COUNT: AtomicU32 = AtomicU32::new(0);

/// A sturct contains the local apic, but implemented [`Send`] and [`Sync`].
pub struct LocalApic(LocalApicOut);

// Implement them
unsafe impl Send for LocalApic {}
unsafe impl Sync for LocalApic {}

pub fn init() {
    // Disable interrupt to avoid something weird
    x86_64::instructions::interrupts::disable();

    // First, close the 8259 PIC
    unsafe {
        Port::new(0x21).write(0xFFu8);
        Port::new(0xA1).write(0xFFu8);
    }

    // Init LAPIC
    let mut lapic = LAPIC.lock();
    unsafe { lapic.0.enable() }
}

/// Invoke EOI
#[inline]
pub fn eoi() {
    unsafe {
        LAPIC.lock().0.end_of_interrupt();
    }
}
