//! The MMIO module.
extern crate alloc;
use alloc::vec::Vec;
use log::warn;
use pci_types::PciAddress;
use spin::{Once, RwLock};
use log::debug;

pub mod pci;
pub mod pcie;

/// The PCI device list.
pub static PCILIST: RwLock<Vec<PciAddress>> = {
    let pcis = Vec::new();
    RwLock::new(pcis)
};

/// Symbol to assign that is this PCIe or not.
pub static IS_PCIE: Once<bool> = Once::new();

pub fn init() {
    // Init PCIe
    IS_PCIE.call_once(|| {
        if let Err(_) = self::pcie::init() {
            warn!("The PCIe initialization has got some errors, falling back to common PCI...");
            self::pci::init();
            false
        } else {
            true
        }
    });

    debug!("Is PCIe: {}, PCI list: {:?}", IS_PCIE.get().unwrap(), PCILIST.read());
}
