use acpi::platform::interrupt::InterruptModel;
use acpi::platform::ProcessorState;
use acpi::AcpiTables;
use alloc::vec::Vec;
use core::panic;
use log::{info, warn};
use spin::Once;

pub mod handler;

use handler::ProkaAcpiHandler;

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

pub static ACPI_INFO: Once<AcpiInfo> = Once::new();

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

    ACPI_INFO.call_once(|| AcpiInfo {
        lapic_address,
        io_apics,
        interrupt_overrides,
        cpus: cpu_info,
    });
}
