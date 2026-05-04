//! The APIC module.
use core::sync::atomic::{AtomicU32, Ordering};
use lazy_static::lazy_static;
use log::{info, trace};
use pic8259::ChainedPics;
use spin::{Mutex, Once};
use x2apic::lapic::{LocalApic as LocalApicOut, LocalApicBuilder, TimerDivide, TimerMode};
use x86_64::instructions::port::Port;

// Constants
pub const XAPIC_BASE: u64 = 0xFFFFe08000000000;
const PIT_FREQ: u32 = 1193182;
const PIT_CTRL: u16 = 0x43;
const PIT_CH0: u16 = 0x40;
const PIT_SET_MODE3: u8 = 0x36;
const RUN_TIMES: u32 = 1;

// Global statics
lazy_static! {
    /// The local APIC.
    pub static ref LAPIC: Mutex<LocalApic> = {
        let mut lapic_cfg = LocalApicBuilder::new();

        // Set XAPIC base (mapped)
        lapic_cfg.set_xapic_base(XAPIC_BASE);

        // Set up timer
        lapic_cfg.timer_divide(TimerDivide::Div16);
        lapic_cfg.timer_initial(0xFFFFFFFF);
        lapic_cfg.timer_mode(TimerMode::OneShot);
        lapic_cfg.timer_vector(0x30);

        // Set up sprious and error handler
        lapic_cfg.spurious_vector(0xF0);
        lapic_cfg.error_vector(0xF1);

        // Build and store
        let lapic = lapic_cfg.build().unwrap();
        Mutex::new(LocalApic(lapic))
    };

    /// The temporary 8259 PIC
    pub static ref PIC: Mutex<ChainedPics> = unsafe {
        let mut pics = ChainedPics::new_contiguous(0x20);
        pics.initialize();
        pics.write_masks(0b1111_1110, 0xff); // IRQ0
        Mutex::new(pics)
    };
}

pub static COUNT: AtomicU32 = AtomicU32::new(0);
pub static LAPIC_FREQ: Once<u32> = Once::new();

/// A sturct contains the local apic, but implemented [`Send`] and [`Sync`].
pub struct LocalApic(LocalApicOut);

// Implement them
unsafe impl Send for LocalApic {}
unsafe impl Sync for LocalApic {}

pub fn init() {
    x86_64::instructions::interrupts::disable();
    unsafe {
        // This init will calibrate the LAPIC timer to 1ms.
        // First, set up the mode of PIT
        let mut ctrl = Port::new(PIT_CTRL);
        let mut ch0 = Port::new(PIT_CH0);
        let divisor = PIT_FREQ / 100;
        ctrl.write(PIT_SET_MODE3);
        ch0.write(divisor as u8);
        ch0.write((divisor >> 8) as u8);

        // And init the PIC
        let pic = PIC.lock();
        drop(pic);

        // Then, open the global APIC
        let mut lapic = LAPIC.lock();
        lapic.0.enable();

        // Do STI and wait for count
        // Todo: Make 1 to 50
        // (idk why the ass PIT only run fucking once aaa)
        x86_64::instructions::interrupts::enable();
        while COUNT.load(Ordering::Relaxed) < RUN_TIMES {
            let count = COUNT.load(Ordering::Relaxed);
            trace!("Count: {}", count);
        }
        x86_64::instructions::interrupts::disable();

        // Calculate the lapic bus frequency
        let count = lapic.0.timer_current();
        let delta = 0xFFFFFFFF - count;
        let freq = (delta * 16 * 100) as u32 / RUN_TIMES;
        info!("Frequency of LAPIC bus: {}", freq);

        // Store it into the global variable
        LAPIC_FREQ.call_once(|| freq);

        // Reset the LAPIC timer mode
        let initial = freq / 16000; // div=16
        lapic.0.set_timer_mode(TimerMode::Periodic);
        lapic.0.set_timer_initial(initial);
    }
}

/// Invoke EOI
#[inline]
pub fn eoi() {
    unsafe {
        LAPIC.lock().0.end_of_interrupt();
    }
}
