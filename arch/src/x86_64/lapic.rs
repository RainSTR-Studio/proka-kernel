//! The x86_64 Local APIC driver.
//!
//! Every CPU owns its *own* Local APIC. To be SMP‑ready this driver keeps one
//! slot per CPU (indexed by the Local APIC ID) instead of a single global, so
//! that each core programs its own timer and sends its own EOI without
//! clobbering the others.
//!
//! The module is deliberately named `lapic` (not `apic`) because the Local
//! APIC is an x86‑specific device; other architectures will bring their own
//! interrupt controller driver under the same `proka_kernel_arch` abstraction.
use core::sync::atomic::AtomicU32;
use spin::RwLock;
use x2apic::lapic::{LocalApic, LocalApicBuilder, TimerDivide, TimerMode, xapic_base};
use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;

// Maximum number of CPUs the per‑CPU LAPIC table can hold. Local APIC IDs are
// 8‑bit on xAPIC (0..=255).
pub const MAX_CPUS: usize = 256;

// Per‑CPU Local APIC slots, keyed by the CPU's Local APIC ID.
static LAPIC_PER_CPU: [RwLock<Option<LocalApic>>; MAX_CPUS] =
    [const { RwLock::new(None::<LocalApic>) }; MAX_CPUS];

// Per‑CPU APIC timer/calibration tick counters, keyed by APIC ID.
pub static COUNT: [AtomicU32; MAX_CPUS] = [const { AtomicU32::new(0) }; MAX_CPUS];

/// Read the Local APIC ID of the CPU we are currently running on.
///
/// The ID is taken straight from the hardware (x2APIC MSR or xAPIC MMIO
/// register), which is what makes the per‑CPU lookup work on any core.
pub fn apic_id() -> usize {
    unsafe {
        // IA32_APIC_BASE (MSR 0x1B).
        let apic_base = Msr::new(0x1B).read();
        let x2apic_enabled = apic_base & (1 << 10) != 0;
        if x2apic_enabled {
            // x2APIC: the x2APIC ID lives in the low byte of MSR 0x802.
            let id = Msr::new(0x802).read() as u32;
            (id & 0xFF) as usize
        } else {
            // xAPIC: the APIC ID lives in the high byte (bits 24..31) of the
            // MMIO ID register.
            let base = (apic_base & !0xFFF) as usize;
            let id = core::ptr::read_volatile((base + 0x20) as *const u32);
            ((id >> 24) & 0xFF) as usize
        }
    }
}

/// Initialize the Local APIC of the boot CPU and start its periodic timer.
pub fn init() {
    // Disable interrupts to avoid weird things during setup.
    x86_64::instructions::interrupts::disable();

    // Mask the legacy 8259 PICs.
    unsafe {
        Port::new(0x21).write(0xFFu8);
        Port::new(0xA1).write(0xFFu8);
    }

    let lapic = unsafe {
        let mut lapic_cfg = LocalApicBuilder::new();
        lapic_cfg.set_xapic_base(xapic_base());
        lapic_cfg.timer_divide(TimerDivide::Div16);
        lapic_cfg.timer_initial(20000);
        lapic_cfg.timer_mode(TimerMode::Periodic);
        lapic_cfg.timer_vector(0x30);
        lapic_cfg.spurious_vector(0xF0);
        lapic_cfg.error_vector(0xF1);
        lapic_cfg.build().unwrap()
    };

    // Resolve the slot from the hardware before enabling (apic_id() reads the
    // MMIO ID register if x2APIC is not enabled yet, which is always safe).
    let id = apic_id();
    unsafe {
        *LAPIC_PER_CPU[id].write() = Some(lapic);
        // Configure + unmask the timer, then software‑enable the LAPIC.
        LAPIC_PER_CPU[id].write().as_mut().unwrap().enable();
    }
}

/// Acknowledge the pending interrupt on the *current* CPU's Local APIC (EOI).
#[inline]
pub fn eoi() {
    let id = apic_id();
    unsafe {
        if let Some(lapic) = LAPIC_PER_CPU[id].write().as_mut() {
            lapic.end_of_interrupt();
        }
    }
}
