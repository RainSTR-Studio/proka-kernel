use crate::get_hhdm_offset;
use crate::interrupts::idt::SPURIOUS_APIC_VECTOR;
use crate::interrupts::pic;
use crate::libs::msr;
use log::{debug, error, info};
use raw_cpuid::CpuId;
use x86_64::registers::model_specific::Msr;

const APIC_SIVR_ENABLE: u64 = 0x100;
const APIC_BASE_ENABLE: u64 = 1 << 11;
const APIC_BASE_X2APIC_ENABLE: u64 = 1 << 10;

// MMIO Offsets for xAPIC
const XAPIC_EOI_OFFSET: u64 = 0x0B0;
const XAPIC_SIVR_OFFSET: u64 = 0xF0;
pub fn apic_is_available() -> bool {
    let cpuid = CpuId::new();
    cpuid.get_feature_info().is_some_and(|info| info.has_apic())
}

pub fn x2apic_is_available() -> bool {
    let cpuid = CpuId::new();
    cpuid
        .get_feature_info()
        .is_some_and(|info| info.has_x2apic())
}

/// Enables x2APIC mode
pub fn enable_x2apic() {
    unsafe {
        // Step 1: Enable x2APIC mode in IA32_APIC_BASE
        let mut apic_base = Msr::new(msr::IA32_APIC_BASE);
        let mut base_val = apic_base.read();
        base_val |= APIC_BASE_ENABLE | APIC_BASE_X2APIC_ENABLE;
        apic_base.write(base_val);

        // Step 2: Set SIVR
        let mut sivr = Msr::new(msr::IA32_X2APIC_SIVR);
        let sivr_val = APIC_SIVR_ENABLE | SPURIOUS_APIC_VECTOR as u64;
        sivr.write(sivr_val);
    }
}

/// Helper to get the virtual base address of the Local APIC (xAPIC mode)
unsafe fn get_xapic_base() -> u64 {
    let apic_base = Msr::new(msr::IA32_APIC_BASE).read();
    let phys_base = apic_base & 0xFFFFF000; // Frame aligned
    let hhdm_offset = get_hhdm_offset().as_u64();
    phys_base + hhdm_offset
}

/// Enables legacy xAPIC mode
pub fn enable_apic() {
    unsafe {
        // Step 1: Ensure Global Enable in IA32_APIC_BASE
        let mut apic_base = Msr::new(msr::IA32_APIC_BASE);
        let mut base_val = apic_base.read();
        base_val |= APIC_BASE_ENABLE;
        base_val &= !APIC_BASE_X2APIC_ENABLE;
        apic_base.write(base_val);

        // Step 2: Set SIVR via MMIO
        let base_addr = get_xapic_base();
        let sivr_ptr = (base_addr + XAPIC_SIVR_OFFSET) as *mut u32;
        let sivr_val = (APIC_SIVR_ENABLE | SPURIOUS_APIC_VECTOR as u64) as u32;
        core::ptr::write_volatile(sivr_ptr, sivr_val);
    }
}

/// Signal End of Interrupt (EOI) to Local APIC
pub fn end_of_interrupt() {
    unsafe {
        if x2apic_is_available() {
            let mut eoi = Msr::new(msr::IA32_X2APIC_EOI);
            eoi.write(0);
        } else {
            let base_addr = get_xapic_base();
            let eoi_ptr = (base_addr + XAPIC_EOI_OFFSET) as *mut u32;
            core::ptr::write_volatile(eoi_ptr, 0);
        }
    }
}

pub fn init() -> bool {
    // Always disable 8259 PIC when using APIC
    pic::disable();

    let success = if x2apic_is_available() {
        debug!("x2APIC is available, enabling...");
        enable_x2apic();
        info!("x2APIC enabled");
        true
    } else if apic_is_available() {
        debug!("x2APIC not available, falling back to xAPIC...");
        enable_apic();
        info!("xAPIC enabled");
        true
    } else {
        error!("APIC not supported!");
        false
    };

    success
}
