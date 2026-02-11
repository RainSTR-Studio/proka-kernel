use core::ptr::NonNull;

use crate::get_hhdm_offset;
use acpi::{Handler, PciAddress, PhysicalMapping};
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, Size4KiB, Translate};
use x86_64::{PhysAddr, VirtAddr};

#[derive(Clone)]
pub struct ProkaAcpiHandler;

impl Handler for ProkaAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let hhdm_offset = get_hhdm_offset();
        let virtual_address = physical_address + hhdm_offset.as_u64() as usize;

        let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
        if let Some(ms) = ms_lock.as_mut() {
            let start_addr = VirtAddr::new(virtual_address as u64);
            let end_addr = VirtAddr::new((virtual_address + size) as u64);

            let start_page = Page::<Size4KiB>::containing_address(start_addr);
            let end_page = Page::<Size4KiB>::containing_address(end_addr - 1u64);

            for page in Page::range_inclusive(start_page, end_page) {
                if ms.page_table.translate_addr(page.start_address()).is_none() {
                    let phys = PhysAddr::new(page.start_address().as_u64() - hhdm_offset.as_u64());

                    let mut frame_allocator = crate::memory::FRAME_ALLOCATOR;

                    let flags = PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE;
                    ms.page_table
                        .map_to(
                            page,
                            x86_64::structures::paging::PhysFrame::containing_address(phys),
                            flags,
                            &mut frame_allocator,
                        )
                        .unwrap()
                        .flush();
                }
            }
        }

        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(virtual_address as *mut T).unwrap(),
            region_length: size,
            mapped_length: size,
            handler: Self,
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}

    fn read_u8(&self, address: usize) -> u8 {
        unsafe { *((address + get_hhdm_offset().as_u64() as usize) as *const u8) }
    }
    fn read_u16(&self, address: usize) -> u16 {
        unsafe { *((address + get_hhdm_offset().as_u64() as usize) as *const u16) }
    }
    fn read_u32(&self, address: usize) -> u32 {
        unsafe { *((address + get_hhdm_offset().as_u64() as usize) as *const u32) }
    }
    fn read_u64(&self, address: usize) -> u64 {
        unsafe { *((address + get_hhdm_offset().as_u64() as usize) as *const u64) }
    }

    fn write_u8(&self, address: usize, value: u8) {
        unsafe {
            *((address + get_hhdm_offset().as_u64() as usize) as *mut u8) = value;
        }
    }
    fn write_u16(&self, address: usize, value: u16) {
        unsafe {
            *((address + get_hhdm_offset().as_u64() as usize) as *mut u16) = value;
        }
    }
    fn write_u32(&self, address: usize, value: u32) {
        unsafe {
            *((address + get_hhdm_offset().as_u64() as usize) as *mut u32) = value;
        }
    }
    fn write_u64(&self, address: usize, value: u64) {
        unsafe {
            *((address + get_hhdm_offset().as_u64() as usize) as *mut u64) = value;
        }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { x86_64::instructions::port::Port::<u8>::new(port).read() }
    }
    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { x86_64::instructions::port::Port::<u16>::new(port).read() }
    }
    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { x86_64::instructions::port::Port::<u32>::new(port).read() }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { x86_64::instructions::port::Port::<u8>::new(port).write(value) }
    }
    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { x86_64::instructions::port::Port::<u16>::new(port).write(value) }
    }
    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { x86_64::instructions::port::Port::<u32>::new(port).write(value) }
    }

    fn read_pci_u8(&self, _address: PciAddress, _offset: u16) -> u8 {
        unimplemented!()
    }
    fn read_pci_u16(&self, _address: PciAddress, _offset: u16) -> u16 {
        unimplemented!()
    }
    fn read_pci_u32(&self, _address: PciAddress, _offset: u16) -> u32 {
        unimplemented!()
    }

    fn write_pci_u8(&self, _address: PciAddress, _offset: u16, _value: u8) {
        unimplemented!()
    }
    fn write_pci_u16(&self, _address: PciAddress, _offset: u16, _value: u16) {
        unimplemented!()
    }
    fn write_pci_u32(&self, _address: PciAddress, _offset: u16, _value: u32) {
        unimplemented!()
    }

    fn nanos_since_boot(&self) -> u64 {
        (crate::libs::time::time_since_boot() * 1_000_000_000.0) as u64
    }
    fn stall(&self, microseconds: u64) {
        crate::libs::time::sleep_us(microseconds)
    }
    fn sleep(&self, milliseconds: u64) {
        crate::libs::time::sleep_us(milliseconds * 1000)
    }

    fn create_mutex(&self) -> acpi::Handle {
        acpi::Handle(0)
    }
    fn acquire(&self, _handle: acpi::Handle, _timeout: u16) -> Result<(), acpi::aml::AmlError> {
        Ok(())
    }
    fn release(&self, _handle: acpi::Handle) {}
}
