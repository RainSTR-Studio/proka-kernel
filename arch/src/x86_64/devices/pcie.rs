//! The architecture-specified PCIe module.
extern crate alloc;
use crate::{
    acpi::ACPI_PLATFORM,
    memory::{MAPPER, framealloc::FRAME_ALLOCATOR},
};
use acpi::sdt::mcfg::Mcfg;
use proka_kernel_common::devices::pcie::PcieCfgAccess;
use x86_64::{
    PhysAddr, align_down, align_up,
    structures::paging::{Mapper, PageTableFlags, PhysFrame, Size2MiB, mapper::MapToError},
};

/// PCIe MMIO mapper.
#[allow(clippy::result_unit_err)]
pub fn map_pcie(base: u64, start: u8, end: u8) -> Result<(), ()> {
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
