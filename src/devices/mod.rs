//! The device init module.
//!
//! The PCI device list and the "PCIe or legacy PCI" flag are
//! architecture-independent and live in `proka_kernel_core::devices`;
//! this module re-exports them and keeps the probing orchestration.
extern crate alloc;
use log::debug;
use log::warn;
pub use proka_kernel_core::devices::{IS_PCIE, PCILIST};
pub mod pcie;

pub fn init() {
    // Init PCIe
    IS_PCIE.call_once(|| {
        if self::pcie::init().is_err() {
            #[cfg(target_arch = "x86_64")]
            {
                warn!("The PCIe initialization has got some errors, falling back to common PCI...");
                proka_kernel_arch::devices::pci::init();
                false
            }

            #[cfg(not(target_arch = "x86_64"))]
            panic!("Could not initialize PCIe");
        } else {
            true
        }
    });

    debug!(
        "Is PCIe: {}, PCI list: {:?}",
        IS_PCIE.get().unwrap(),
        PCILIST.read()
    );
}
