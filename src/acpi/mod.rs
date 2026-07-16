//! The ACPI module
pub mod power;

// According to the documentation of the [`acpi`] crate,
// we first need to implement a [`Handler`] trait.

use crate::{
    devices::{IS_PCIE, pci::PciCfgAccess, pcie::get_access},
    memory::{MAPPER, framealloc::FRAME_ALLOCATOR},
};
use acpi::{AcpiTables, Handle, Handler, aml::Interpreter, platform::AcpiPlatform};
use core::ptr::NonNull;
use pci_types::ConfigRegionAccess;
use spin::LazyLock;
use x86_64::{
    PhysAddr, align_up,
    instructions::port::Port,
    structures::paging::{Mapper, PageTableFlags, PhysFrame, Size2MiB, mapper::MapToError},
};

/// The ACPI Root table.
pub static ACPI_PLATFORM: LazyLock<AcpiPlatform<AcpiHandler>> = LazyLock::new(|| unsafe {
    let addr = proka_bootloader::get_bootinfo().acpi() as usize;
    let acpi = AcpiTables::from_rsdp(AcpiHandler, addr).expect("ACPI table init failed");

    AcpiPlatform::new(acpi, AcpiHandler).expect("Failed to init ACPI platform")
});

/// The AML interpreter.
pub static AMLINT: LazyLock<Interpreter<AcpiHandler>> = LazyLock::new(|| {
    let interpreter = Interpreter::new_from_platform(&ACPI_PLATFORM);
    interpreter.expect("Failed to load AML interpreter")
});

/// The ACPI handler.
#[derive(Debug, Clone, Copy)]
pub struct AcpiHandler;

// Implementations
// TODO: Implement more method
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
        let size_aligned = align_up(size as u64, 0x200000) as usize;
        let range = size_aligned >> 21;
        let flags = PageTableFlags::PRESENT;
        for addr in physical_address..=physical_address + range {
            let phys = PhysAddr::new(addr as u64);
            let frame = PhysFrame::<Size2MiB>::containing_address(phys);
            match unsafe { mapper.identity_map(frame, flags, &mut *framealloc) } {
                Ok(flusher) => flusher.flush(),
                Err(MapToError::PageAlreadyMapped(_)) => (),
                Err(e) => panic!(
                    "Mapping ACPI process NOT successfully!\n\
                    Occurs in mapping page {:08x}, size {}, problem is {:?}.",
                    physical_address, size, e
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

    fn read_pci_u16(&self, address: acpi::PciAddress, offset: u16) -> u16 {
        // Check is this PCIe...
        if *IS_PCIE.get().unwrap() {
            // Here we'd like to use PCIe method
            // To read this, we need to get the config access...
            let access = get_access(address.segment()).expect("No! This segment not exist!");
            let value = unsafe { access.read(address, offset) };
            value as u16
        } else {
            // We have to use PCI.
            let access = PciCfgAccess;
            unsafe { access.read(address, offset) as u16 }
        }
    }

    fn read_pci_u32(&self, address: acpi::PciAddress, offset: u16) -> u32 {
        // Check is this PCIe...
        if *IS_PCIE.get().unwrap() {
            // Here we'd like to use PCIe method
            // To read this, we need to get the config access...
            let access = get_access(address.segment()).expect("No! This segment not exist!");
            unsafe { access.read(address, offset) }
        } else {
            // We have to use PCI.
            let access = PciCfgAccess;
            unsafe { access.read(address, offset) }
        }
    }

    fn read_pci_u8(&self, address: acpi::PciAddress, offset: u16) -> u8 {
        // Check is this PCIe...
        if *IS_PCIE.get().unwrap() {
            // Here we'd like to use PCIe method
            // To read this, we need to get the config access...
            let access = get_access(address.segment()).expect("No! This segment not exist!");
            let value = unsafe { access.read(address, offset) };
            value as u8
        } else {
            // We have to use PCI.
            let access = PciCfgAccess;
            unsafe { access.read(address, offset) as u8 }
        }
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

    fn write_pci_u32(&self, address: acpi::PciAddress, offset: u16, value: u32) {
        // Check is this PCIe...
        if *IS_PCIE.get().unwrap() {
            // Here we'd like to use PCIe method
            // To write this, we need to get the config access...
            let access = get_access(address.segment()).expect("No! This segment not exist!");
            unsafe { access.write(address, offset, value) };
        } else {
            // We have to use PCI.
            let access = PciCfgAccess;
            unsafe { access.write(address, offset, value) };
        }
    }

    fn write_pci_u16(&self, address: acpi::PciAddress, offset: u16, value: u16) {
        // Check is this PCIe...
        if *IS_PCIE.get().unwrap() {
            // Here we'd like to use PCIe method
            // To write this, we need to get the config access...
            let access = get_access(address.segment()).expect("No! This segment not exist!");
            unsafe { access.write(address, offset, value as u32) };
        } else {
            // We have to use PCI.
            let access = PciCfgAccess;
            unsafe { access.write(address, offset, value as u32) };
        }
    }

    fn write_pci_u8(&self, address: acpi::PciAddress, offset: u16, value: u8) {
        // Check is this PCIe...
        if *IS_PCIE.get().unwrap() {
            // Here we'd like to use PCIe method
            // To write this, we need to get the config access...
            let access = get_access(address.segment()).expect("No! This segment not exist!");
            unsafe { access.write(address, offset, value as u32) };
        } else {
            // We have to use PCI.
            let access = PciCfgAccess;
            unsafe { access.write(address, offset, value as u32) };
        }
    }

    fn stall(&self, _microseconds: u64) {}
    fn sleep(&self, _milliseconds: u64) {}
    fn release(&self, _mutex: Handle) {}
}

/// ACPI initializator.
pub fn init() {
    // Enable ACPI mode
    ACPI_PLATFORM
        .enter_acpi_mode()
        .expect("Failed to enable ACPI mode");

    // Enable AML interpreter
    AMLINT.initialize_namespace();
}
