//! The APIC module.
use log::trace;
use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;

// Constants
pub const LAPIC_SPURIOUS: u32 = 0xF0;
pub const LAPIC_LVT_TIMER: u32 = 0x320;
pub const LAPIC_TIMER_DIV: u32 = 0x3E0;
pub const LAPIC_TIMER_INIT: u32 = 0x380;
pub const LAPIC_EOI: u32 = 0xB0;

// Local APIC enable | Periodic timer
const APIC_ENABLE: u32 = 1 << 8;
const TIMER_PERIODIC: u32 = 1 << 17;

pub fn init() {
    unsafe {
        // First, close the 8259 PIC
        Port::new(0x21).write(0xFFu8);
        Port::new(0xA1).write(0xFFu8);

        // Then, open the global APIC
        let mut base = Msr::new(0x1B).read();
        trace!("{base}");
        base |= 1 << 11;
        Msr::new(0x1B).write(base);

        // Enable Local APIC + spurious interrupt vector 0xFF
        lapic_write(LAPIC_SPURIOUS, APIC_ENABLE | 0xFF);

        // Timer divide by 16
        lapic_write(LAPIC_TIMER_DIV, 0x3);

        // Periodic timer, interrupt vector 0x30
        lapic_write(LAPIC_LVT_TIMER, TIMER_PERIODIC | 0x30);

        // Set initial count (ticks periodic timer)
        lapic_write(LAPIC_TIMER_INIT, 100000);

        x86_64::instructions::interrupts::enable();
    }
}

/// Write to LAPIC MMIO
#[inline]
fn lapic_write(offset: u32, value: u32) {
    let base = 0xFFFFe08000000000;
    unsafe {
        ((base + offset as u64) as *mut u32)
            .write_volatile(value);
    }
}

/// Invoke EOI
#[inline]
pub fn eoi() {
    lapic_write(LAPIC_EOI, 0);
}
