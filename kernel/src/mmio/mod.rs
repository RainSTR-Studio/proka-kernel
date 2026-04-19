//! The MMIO Manager.
use log::debug;
pub mod pci;

/// MMIO Initializator.
pub fn init() {
    // Scan PCI
    debug!("=====Begin of PCI device list=====");
    self::pci::pci_scan();
    debug!("=====End of PCI device list=====");
}
