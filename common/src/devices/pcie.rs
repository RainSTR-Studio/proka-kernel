//! The PCIe generic definition module.
use pci_types::{ConfigRegionAccess, PciAddress};

/// The implementation of config region access (PCIe).
#[derive(Debug, Clone, Copy)]
pub struct PcieCfgAccess(u64);

// Common impl
impl PcieCfgAccess {
    /// Create new cfg address.
    pub const fn new(base: u64) -> Self {
        Self(base)
    }

    /// Get the base address.
    pub const fn address(&self) -> u64 {
        self.0
    }
}

impl ConfigRegionAccess for PcieCfgAccess {
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        // Calc the exact base now..
        let pci_base = self.0
            + address.bus() as u64 * 0x100000
            + address.device() as u64 * 0x8000
            + address.function() as u64 * 0x1000;
        let offset_aligned = offset & 0xfc;
        let exact_addr = pci_base + offset_aligned as u64;

        // Convert to raw pointer.
        let ptr = exact_addr as *const u32;
        unsafe { *ptr }
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let pci_base = self.0
            + address.bus() as u64 * 0x100000
            + address.device() as u64 * 0x8000
            + address.function() as u64 * 0x1000;
        let offset_aligned = offset & 0xfc;
        let exact_addr = pci_base + offset_aligned as u64;

        // Convert to raw pointer.
        let ptr = exact_addr as *mut u32;
        unsafe { *ptr = value }
    }
}
