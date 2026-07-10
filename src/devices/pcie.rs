//! The PCIe module.
extern crate alloc;
use super::PCILIST;
use crate::{
    acpi::ACPI_PLATFORM,
    memory::{MAPPER, framealloc::FRAME_ALLOCATOR},
};
use acpi::sdt::mcfg::Mcfg;
use pci_types::{ConfigRegionAccess, PciAddress, PciHeader};
use x86_64::{
    PhysAddr, align_down, align_up,
    structures::paging::{Mapper, PageTableFlags, PhysFrame, Size2MiB, mapper::MapToError},
};

/// The implementation of config region access (PCIe).
#[derive(Debug, Clone, Copy)]
pub struct PcieCfgAccess(u64);

// Common impl
impl PcieCfgAccess {
    /// Create new cfg address.
    pub const fn new(base: u64) -> Self {
        Self(base)
    }

    /// Get the base address.
    pub const fn address(&self) -> u64 {
        self.0
    }
}

impl ConfigRegionAccess for PcieCfgAccess {
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        // Calc the exact base now..
        let pci_base = self.0
            + address.bus() as u64 * 0x100000
            + address.device() as u64 * 0x8000
            + address.function() as u64 * 0x1000;
        let exact_addr = pci_base + offset as u64;

        // Convert to raw pointer.
        let ptr = exact_addr as *const u32;
        unsafe { *ptr }
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let pci_base = self.0
            + address.bus() as u64 * 0x100000
            + address.device() as u64 * 0x8000
            + address.function() as u64 * 0x1000;
        let exact_addr = pci_base + offset as u64;

        // Convert to raw pointer.
        let ptr = exact_addr as *mut u32;
        unsafe { *ptr = value }
    }
}

pub fn init() -> Result<(), ()> {
    // First of all, we need to find the PCIe's base address
    // So, we shall read the ACPI table (MCFG).
    let mcfg = ACPI_PLATFORM.tables.find_table::<Mcfg>().ok_or(())?;
    for entry in mcfg.entries() {
        // Get each essential info
        let base = entry.base_address;
        let start = entry.bus_number_start;
        let end = entry.bus_number_end;
        let segment = entry.pci_segment_group;
        let access = PcieCfgAccess::new(base);

        // Each bus's length is 1MiB, so let's calc the address that we should map.
        let addr_start = base + start as u64 * 0x100000;
        let addr_end = base + (end - start) as u64 * 0x100000;

        // As each page size is 2MiB, we should align them as 2MiB address.
        let addr_start_aligned = align_down(addr_start, 0x200000);
        let addr_end_aligned = align_up(addr_end, 0x200000);

        // So that we can calc the total pages and do iteration mapping.
        let pages = (addr_end_aligned - addr_start_aligned) / 0x200000; // Each page size = 2MiB

        // Map each PCIe address.
        for i in 0..pages {
            let mut frame_alloc = FRAME_ALLOCATOR.lock();
            let mut mapper = MAPPER.lock();
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            let frame = PhysFrame::<Size2MiB>::containing_address(PhysAddr::new(
                addr_start_aligned + i * 0x200000,
            ));
            unsafe {
                let result = mapper.identity_map(frame, flags, &mut *frame_alloc);
                match result {
                    Ok(flusher) => flusher.flush(),
                    Err(MapToError::PageAlreadyMapped(_)) => (),
                    Err(_) => Err(())?,
                }
            }
        }

        // As we have mapped the address, we can fill out the PCI list.
        // It's time to do iteration for each buses, devices and functions.
        for bus in start..end {
            for device in 0..32u8 {
                for function in 0..8u8 {
                    let address = PciAddress::new(segment, bus, device, function);

                    // Check: Is this PCI address valid
                    let header = PciHeader::new(address);
                    let id = header.id(access);
                    if id.0 != 0xffff {
                        PCILIST.write().push(address);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Get the config access by passing a segment.
pub fn get_access(segment: u16) -> Option<PcieCfgAccess> {
    // Get MCFG table.
    let mcfg = ACPI_PLATFORM.tables.find_table::<Mcfg>()?;

    // Iterate and match...
    for entry in mcfg.entries() {
        if entry.pci_segment_group == segment {
            // Nice!! I found that!
            return Some(PcieCfgAccess::new(entry.base_address));
        }
    }

    // No! Not found :(
    None
}
