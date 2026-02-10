use crate::get_hhdm_offset;
use acpi::platform::interrupt::InterruptModel;
use acpi::platform::ProcessorState;
use acpi::{AcpiTables, Handler, PciAddress, PhysicalMapping};
use alloc::vec::Vec;
use core::panic;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use log::{info, warn};
use spin::Mutex;
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

pub struct IoApicInfo {
    pub id: u8,
    pub address: u32,
    pub global_system_interrupt_base: u32,
}

pub struct InterruptOverride {
    pub source: u8,
    pub mapped_to: u32,
}

pub struct CpuInfo {
    pub id: u32,
    pub apic_id: u32,
    pub enabled: bool,
}

pub struct AcpiInfo {
    pub lapic_address: u64,
    pub io_apics: Vec<IoApicInfo>,
    pub interrupt_overrides: Vec<InterruptOverride>,
    pub cpus: Vec<CpuInfo>,
}

lazy_static! {
    pub static ref ACPI_INFO: Mutex<Option<AcpiInfo>> = Mutex::new(None);
}

pub fn init() {
    let rsdp_addr = crate::RSDP_REQUEST.get_response().and_then(|r| {
        let addr = r.address() as usize;
        if addr == 0 {
            None
        } else {
            Some(addr)
        }
    });

    let Some(rsdp_addr) = rsdp_addr else {
        panic!("ACPI RSDP not found");
    };

    info!("ACPI RSDP found at {:#x}", rsdp_addr);

    let handler = ProkaAcpiHandler;
    let tables =
        unsafe { AcpiTables::from_rsdp(handler, rsdp_addr).expect("Failed to parse ACPI tables") };

    let (interrupt_model, processor_info) =
        InterruptModel::new(&tables).expect("Failed to get interrupt model");

    let mut lapic_address = 0;
    let mut io_apics = Vec::new();
    let mut interrupt_overrides = Vec::new();

    if let InterruptModel::Apic(apic) = interrupt_model {
        lapic_address = apic.local_apic_address;
        for io_apic in apic.io_apics {
            io_apics.push(IoApicInfo {
                id: io_apic.id,
                address: io_apic.address,
                global_system_interrupt_base: io_apic.global_system_interrupt_base,
            });
        }
        for irq_override in apic.interrupt_source_overrides {
            interrupt_overrides.push(InterruptOverride {
                source: irq_override.isa_source,
                mapped_to: irq_override.global_system_interrupt,
            });
        }
    } else {
        warn!("APIC interrupt model not found in ACPI");
    }

    info!("LAPIC Address: {:#x}", lapic_address);
    info!("Found {} I/O APICs", io_apics.len());

    let mut cpu_info = Vec::new();
    if let Some(processor_info) = processor_info {
        info!(
            "Found {} CPUs",
            processor_info.application_processors.len() + 1
        );
        let bsp = processor_info.boot_processor;
        cpu_info.push(CpuInfo {
            id: bsp.processor_uid,
            apic_id: bsp.local_apic_id,
            enabled: bsp.state == ProcessorState::Running,
        });
        for cpu in processor_info.application_processors {
            cpu_info.push(CpuInfo {
                id: cpu.processor_uid,
                apic_id: cpu.local_apic_id,
                enabled: cpu.state == ProcessorState::Running,
            });
        }

        if cpu_info.len() as u32 > u32::MAX {
            panic!("You're GOAT. 4.2 billion CPUs?");
        }
    } else {
        warn!("Where is your CPU?");
        warn!("No processor info found in ACPI");
    }

    let mut info_lock = ACPI_INFO.lock();
    *info_lock = Some(AcpiInfo {
        lapic_address,
        io_apics,
        interrupt_overrides,
        cpus: cpu_info,
    });
}
