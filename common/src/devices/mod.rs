//! Architecture-independent device enumeration data.
//!
//! PCI is a bus standard shared by all supported architectures, so the
//! discovered device list and the "PCIe or legacy PCI" flag are kept here.
//! The actual probing (PCIe config-space mapping / legacy port IO) is
//! architecture-specific and stays in `proka-kernel-arch`.
pub mod pcie;
extern crate alloc;
use alloc::vec::Vec;
use pci_types::PciAddress;
use spin::{Once, RwLock};

/// The PCI device list.
pub static PCILIST: RwLock<Vec<PciAddress>> = {
    let pcis = Vec::new();
    RwLock::new(pcis)
};

/// Symbol to assign that is this PCIe or not.
pub static IS_PCIE: Once<bool> = Once::new();
