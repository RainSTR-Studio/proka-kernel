use crate::libs::acpi::ACPI_INFO;
use crate::{get_hhdm_offset, memory::protection};
use alloc::vec::Vec;
use log::{debug, info};
use spin::Once;

const IOREGSEL: u64 = 0x00;
const IOWIN: u64 = 0x10;

const IOAPICVER: u32 = 0x01;
const IOREDTBL: u32 = 0x10;

pub struct IoApic {
    base: u64,
    gsi_base: u32,
    max_redirection_entries: u8,
}

pub static IOAPICS: Once<Vec<IoApic>> = Once::new();

impl IoApic {
    pub unsafe fn new(_id: u8, phys_base: u64, gsi_base: u32) -> Self {
        let base = phys_base + get_hhdm_offset().as_u64();

        // Map the I/O APIC MMIO region
        {
            let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
            if let Some(ms) = ms_lock.as_mut() {
                ms.map_region(
                    x86_64::VirtAddr::new(base),
                    x86_64::PhysAddr::new(phys_base),
                    0x1000,
                    protection::ioapic_flags(),
                )
                .unwrap();
            }
        }

        let mut ioapic = Self {
            base,
            gsi_base,
            max_redirection_entries: 0,
        };

        let ver = ioapic.read(IOAPICVER);
        ioapic.max_redirection_entries = ((ver >> 16) & 0xFF) as u8;

        debug!(
            "I/O APIC at {:#x} (GSI Base: {}) version {:#x}, max entries: {}",
            phys_base,
            gsi_base,
            ver & 0xFF,
            ioapic.max_redirection_entries
        );

        ioapic
    }

    pub unsafe fn read(&self, reg: u32) -> u32 {
        let ioregsel = (self.base + IOREGSEL) as *mut u32;
        let iowin = (self.base + IOWIN) as *mut u32;

        core::ptr::write_volatile(ioregsel, reg);
        core::ptr::read_volatile(iowin)
    }

    pub unsafe fn write(&self, reg: u32, value: u32) {
        let ioregsel = (self.base + IOREGSEL) as *mut u32;
        let iowin = (self.base + IOWIN) as *mut u32;

        core::ptr::write_volatile(ioregsel, reg);
        core::ptr::write_volatile(iowin, value);
    }

    pub unsafe fn set_redirection(&self, index: u8, vector: u8, dest_id: u8, flags: u64) {
        if index > self.max_redirection_entries {
            return;
        }

        let low_reg = IOREDTBL + (index as u32) * 2;
        let high_reg = low_reg + 1;

        let low_val = (vector as u32) | (flags as u32);
        let high_val = (dest_id as u32) << 24 | (flags >> 32) as u32;

        self.write(low_reg, low_val);
        self.write(high_reg, high_val);
    }
}

pub fn init() {
    let Some(acpi_info) = ACPI_INFO.get() else {
        panic!("ACPI info not initialized before IOAPIC init");
    };

    let ioapics = IOAPICS.call_once(|| {
        let mut ioapics = Vec::new();
        for info in &acpi_info.io_apics {
            unsafe {
                let ioapic = IoApic::new(
                    info.id,
                    info.address as u64,
                    info.global_system_interrupt_base,
                );
                ioapics.push(ioapic);
            }
        }
        ioapics
    });

    info!("Initialized {} I/O APICs", ioapics.len());
}

pub fn route_irq(irq: u8, vector: u8, dest_id: u8) {
    let Some(acpi_info) = ACPI_INFO.get() else {
        return;
    };

    // Check for overrides
    let mut actual_gsi = irq as u32;
    for ovr in &acpi_info.interrupt_overrides {
        if ovr.source == irq {
            actual_gsi = ovr.mapped_to;
            break;
        }
    }

    let Some(ioapics) = IOAPICS.get() else {
        return;
    };

    for ioapic in ioapics.iter() {
        if actual_gsi >= ioapic.gsi_base
            && actual_gsi <= ioapic.gsi_base + ioapic.max_redirection_entries as u32
        {
            unsafe {
                ioapic.set_redirection((actual_gsi - ioapic.gsi_base) as u8, vector, dest_id, 0);
            }
            return;
        }
    }
}
