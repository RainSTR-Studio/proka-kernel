//! The MMIO module.
extern crate alloc;
use pci_types::PciAddress;
use alloc::vec::Vec;
use proka_bootloader::BootMode;
use spin::RwLock;
pub mod pcie;

/// The PCI device list.
pub static PCILIST: RwLock<Vec<PciAddress>> = {
    let pcis = Vec::new();
    RwLock::new(pcis)
};

pub fn init() {
    // Init PCIe
    if unsafe { proka_bootloader::get_bootinfo().boot_mode() == BootMode::Uefi } {
        self::pcie::init();
    }
}
