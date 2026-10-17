//! The PCIe module.
extern crate alloc;
use super::PCILIST;
use proka_kernel_arch::acpi::ACPI_PLATFORM;
use acpi::sdt::mcfg::Mcfg;
use proka_kernel_common::devices::pcie::PcieCfgAccess;
use pci_types::{PciAddress, PciHeader};

// XXX: Use 2 unit result can make code much more readable
#[allow(clippy::result_unit_err)]
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

        // Map PCIe MMIO space
        proka_kernel_arch::devices::pcie::map_pcie(base, start, end)?;

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
