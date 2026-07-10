//! The traditional PCI module.
use pci_types::{ConfigRegionAccess, PciAddress, PciHeader};
use x86_64::instructions::port::Port;
use super::PCILIST;
const PCI_CFG: u16 = 0xcf8;
const PCI_DATA: u16 = 0xcfc;

/// The implementation of config region access (traditional PCI).
#[derive(Debug, Clone, Copy)]
pub struct PciCfgAccess;

impl ConfigRegionAccess for PciCfgAccess {
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        // Construct a value to 0xcf8...
        let cfg = 0x80000000
            | ((address.bus() as u32) << 16)
            | ((address.device() as u32) << 11)
            | ((address.function() as u32) << 8)
            | ((offset as u32) & 0xfc);
        unsafe {
            Port::new(PCI_CFG).write(cfg);
            Port::new(PCI_DATA).read()
        }
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        // Construct a value to 0xcf8...
        let cfg = 0x80000000
            | ((address.bus() as u32) << 16)
            | ((address.device() as u32) << 11)
            | ((address.function() as u32) << 8)
            | ((offset as u32) & 0xfc);
        unsafe {
            Port::new(PCI_CFG).write(cfg);
            Port::new(PCI_DATA).write(value);
        };
    }
}

/// Init PCI
pub fn init() {
    // Scan 256 buses, 32 devices, 8 functions...
    for bus in 0..=255 {
        for device in 0..=31 {
            for function in 0..=7 {
                let address = PciAddress::new(0, bus, device, function);
                let header = PciHeader::new(address);

                // Check: Is this a valid device?
                if header.id(PciCfgAccess).0 != 0xFFFF {
                    PCILIST.write().push(address);
                }
            }
        }
    }
}
