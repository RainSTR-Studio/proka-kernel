//! The ACPI module

// According to the documentation of the [`acpi`] crate,
// we first need to implement a [`Handler`] trait.

use core::ptr::NonNull;

use crate::memory::{MAPPER, framealloc::FRAME_ALLOCATOR};
use acpi::{AcpiTables, Handle, Handler};
use spin::{Lazy, Mutex};
use x86_64::{
    PhysAddr, align_up,
    instructions::port::Port,
    structures::paging::{Mapper, PageTableFlags, PhysFrame, Size4KiB, mapper::MapToError},
};

/// The ACPI Root table.
pub static ACPI_TABLE: Lazy<Mutex<AcpiTables<AcpiHandler>>> = Lazy::new(|| {
    let addr = proka_bootloader::get_bootinfo().acpi() as usize;
    let acpi = unsafe {
        AcpiTables::from_rsdp(AcpiHandler, addr)
            .expect("ACPI not initialized")
    };
    Mutex::new(acpi)
});

/// The ACPI handler.
#[derive(Debug, Clone, Copy)]
pub struct AcpiHandler;

// Implementations
impl Handler for AcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        // For mapping, we can just use the mapper to do identity mapping.
        let mut framealloc = FRAME_ALLOCATOR.lock();
        let mut mapper = MAPPER.lock();

        // Align up the size
        let size_aligned = align_up(size as u64, 4096) as usize;
        let range = size_aligned >> 12;
        let flags = PageTableFlags::PRESENT;
        for addr in physical_address..=physical_address + range {
            let phys = PhysAddr::new(addr as u64);
            let frame = PhysFrame::<Size4KiB>::containing_address(phys);
            match unsafe { mapper.identity_map(frame, flags, &mut *framealloc) } {
                Ok(flusher) => flusher.flush(),
                Err(MapToError::PageAlreadyMapped(_)) => (),
                _ => panic!(
                    "Mapping ACPI process NOT successfully!\n\
                        Occurs in mapping page {physical_address}, size {size}."
                ),
            }
        }

        acpi::PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(physical_address as *mut T).unwrap(),
            region_length: size,
            mapped_length: size,
            handler: AcpiHandler,
        }
    }

    fn unmap_physical_region<T>(_region: &acpi::PhysicalMapping<Self, T>) {
        // Empty
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { Port::new(port).read() }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { Port::new(port).read() }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { Port::new(port).read() }
    }

    fn read_pci_u16(&self, _address: acpi::PciAddress, _offset: u16) -> u16 {
        0
    }

    fn read_pci_u32(&self, _address: acpi::PciAddress, _offset: u16) -> u32 {
        0
    }

    fn read_pci_u8(&self, _address: acpi::PciAddress, _offset: u16) -> u8 {
        0
    }

    fn read_u8(&self, address: usize) -> u8 {
        unsafe { *(address as *mut u8) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        unsafe { *(address as *mut u16) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        unsafe { *(address as *mut u32) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        unsafe { *(address as *mut u64) }
    }

    fn acquire(&self, _mutex: acpi::Handle, _timeout: u16) -> Result<(), acpi::aml::AmlError> {
        Ok(())
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe {
            Port::new(port).write(value);
        }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe {
            Port::new(port).write(value);
        }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe {
            Port::new(port).write(value);
        }
    }

    fn write_u16(&self, address: usize, value: u16) {
        unsafe { *(address as *mut u16) = value }
    }

    fn write_u32(&self, address: usize, value: u32) {
        unsafe { *(address as *mut u32) = value }
    }

    fn write_u64(&self, address: usize, value: u64) {
        unsafe { *(address as *mut u64) = value }
    }

    fn write_u8(&self, address: usize, value: u8) {
        unsafe { *(address as *mut u8) = value }
    }

    fn nanos_since_boot(&self) -> u64 {
        0
    }

    fn create_mutex(&self) -> acpi::Handle {
        acpi::Handle(0)
    }

    fn write_pci_u32(&self, _address: acpi::PciAddress, _offset: u16, _value: u32) {}
    fn write_pci_u16(&self, _address: acpi::PciAddress, _offset: u16, _value: u16) {}

    fn write_pci_u8(&self, _address: acpi::PciAddress, _offset: u16, _value: u8) {}
    fn release(&self, _mutex: Handle) {}
    fn stall(&self, _microseconds: u64) {}
    fn sleep(&self, _milliseconds: u64) {}
}
