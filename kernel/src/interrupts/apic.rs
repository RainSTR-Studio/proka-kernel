use crate::interrupts::idt::SPURIOUS_APIC_VECTOR;
use crate::interrupts::pic;
use crate::libs::msr;
use crate::memory::protection;
use crate::{get_hhdm_offset, libs::acpi::ACPI_INFO};
use log::{debug, error, info};
use raw_cpuid::CpuId;
use spin::Mutex;
use x86_64::registers::model_specific::Msr;

const APIC_SIVR_ENABLE: u32 = 0x100;
const APIC_BASE_ENABLE: u64 = 1 << 11;
const APIC_BASE_X2APIC_ENABLE: u64 = 1 << 10;

// MMIO Offsets for xAPIC
const XAPIC_EOI_OFFSET: u32 = 0x0B0;
const XAPIC_SIVR_OFFSET: u32 = 0x0F0;
const XAPIC_LVT_TIMER_OFFSET: u32 = 0x320;
const XAPIC_TIMER_INIT_COUNT_OFFSET: u32 = 0x380;
const XAPIC_TIMER_CUR_COUNT_OFFSET: u32 = 0x390;
const XAPIC_TIMER_DIV_CONF_OFFSET: u32 = 0x3E0;

pub const TIMER_VECTOR: u8 = 0x30; // APIC Timer interrupt vector

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

#[derive(Debug, Clone, Copy)]
pub enum ApicMode {
    XApic,
    X2Apic,
}

pub struct LocalApic {
    mode: ApicMode,
    base: u64, // Virtual base address for xAPIC
}

lazy_static::lazy_static! {
    pub static ref LAPIC: Mutex<Option<LocalApic>> = Mutex::new(None);
}

impl LocalApic {
    pub unsafe fn new() -> Self {
        let mode = if x2apic_is_available() {
            ApicMode::X2Apic
        } else {
            let cpu_number = ACPI_INFO
                .lock()
                .as_ref()
                .map(|info| info.cpus.len())
                .unwrap_or(1);
            if cpu_number > 255 {
                panic!("xAPIC is not supported for more than 255 CPUs")
            }
            ApicMode::XApic
        };

        let base = {
            let acpi_lapic_addr = ACPI_INFO.lock().as_ref().map(|info| info.lapic_address);
            let phys_base = acpi_lapic_addr.unwrap_or_else(|| {
                let apic_base_msr = Msr::new(msr::IA32_APIC_BASE).read();
                apic_base_msr & 0xFFFFF000
            });
            let virt_base = phys_base + get_hhdm_offset().as_u64();

            // Map the LAPIC MMIO region
            if let ApicMode::XApic = mode {
                let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
                if let Some(ms) = ms_lock.as_mut() {
                    ms.map_region(
                        x86_64::VirtAddr::new(virt_base),
                        x86_64::PhysAddr::new(phys_base),
                        0x1000, // 4KB
                        protection::ioapic_flags(),
                    )
                    .unwrap();
                }
            }

            virt_base
        };

        Self { mode, base }
    }

    pub unsafe fn read_reg(&self, offset: u32) -> u32 {
        match self.mode {
            ApicMode::XApic => {
                let ptr = (self.base + offset as u64) as *const u32;
                core::ptr::read_volatile(ptr)
            }
            ApicMode::X2Apic => {
                let msr_addr = 0x800 + (offset >> 4);
                Msr::new(msr_addr).read() as u32
            }
        }
    }

    pub unsafe fn write_reg(&self, offset: u32, value: u32) {
        match self.mode {
            ApicMode::XApic => {
                let ptr = (self.base + offset as u64) as *mut u32;
                core::ptr::write_volatile(ptr, value)
            }
            ApicMode::X2Apic => {
                let msr_addr = 0x800 + (offset >> 4);
                Msr::new(msr_addr).write(value as u64);
            }
        }
    }

    pub unsafe fn eoi(&self) {
        match self.mode {
            ApicMode::XApic => self.write_reg(XAPIC_EOI_OFFSET, 0),
            ApicMode::X2Apic => Msr::new(msr::IA32_X2APIC_EOI).write(0),
        }
    }

    pub unsafe fn init(&self) {
        // Enable APIC in IA32_APIC_BASE
        let mut apic_base = Msr::new(msr::IA32_APIC_BASE);
        let mut base_val = apic_base.read();
        base_val |= APIC_BASE_ENABLE;
        if let ApicMode::X2Apic = self.mode {
            base_val |= APIC_BASE_X2APIC_ENABLE;
        }
        apic_base.write(base_val);

        // Set SIVR
        let sivr_val = APIC_SIVR_ENABLE | SPURIOUS_APIC_VECTOR as u32;
        self.write_reg(XAPIC_SIVR_OFFSET, sivr_val);

        debug!("Local APIC initialized in {:?} mode", self.mode);
    }

    // TODO: move to libs/time
    pub unsafe fn calibrate_timer(&self) {
        // Stop timer
        self.write_reg(XAPIC_TIMER_INIT_COUNT_OFFSET, 0);
        // Set divider to 16
        self.write_reg(XAPIC_TIMER_DIV_CONF_OFFSET, 0x3);

        let mut pit = crate::libs::time::pit::PIT.lock();

        // Start PIT one-shot for 10ms (10000 us)
        // PIT freq is 1.193182 MHz. 10ms = 11932 ticks
        let pit_ticks = 11932;
        pit.start_one_shot(pit_ticks);

        // Set APIC timer to max
        self.write_reg(XAPIC_TIMER_INIT_COUNT_OFFSET, 0xFFFFFFFF);

        // Wait for PIT
        while (x86_64::instructions::port::Port::<u8>::new(0x61).read() & 0x20) == 0 {
            core::hint::spin_loop();
        }

        // Stop APIC timer
        let current_count = self.read_reg(XAPIC_TIMER_CUR_COUNT_OFFSET);
        let ticks_per_10ms = 0xFFFFFFFF - current_count;

        info!("APIC Timer calibrated: {} ticks per 10ms", ticks_per_10ms);

        // Set timer for periodic interrupt at 100Hz (10ms)
        self.write_reg(XAPIC_LVT_TIMER_OFFSET, 0x20000 | TIMER_VECTOR as u32); // Periodic mode
        self.write_reg(XAPIC_TIMER_INIT_COUNT_OFFSET, ticks_per_10ms);
    }
}

pub fn end_of_interrupt() {
    if let Some(lapic) = LAPIC.lock().as_ref() {
        unsafe { lapic.eoi() };
    }
}

pub fn init() {
    pic::disable();

    if !apic_is_available() {
        panic!("APIC not supported!")
    }

    unsafe {
        let lapic = LocalApic::new();
        lapic.init();
        lapic.calibrate_timer();
        *LAPIC.lock() = Some(lapic);
    }
}
