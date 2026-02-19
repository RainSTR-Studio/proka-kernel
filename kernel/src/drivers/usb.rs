//! The USB driver (xHCI).
//!
//! Copyright (C) RainSTR Studio 2025-2026, All Rights Reserved.

extern crate alloc;
use crate::libs::pci::scan_all_pci_devices;
use alloc::vec::Vec;
use log::warn;

pub fn init() {
    let all_pci_devices = scan_all_pci_devices();

    // Get xHCI devices
    let usb_devices = all_pci_devices
        .iter()
        .filter(|(_, class)| class.class == 0x0C && class.subclass == 0x03 && class.prog_if == 0x30)
        .collect::<Vec<_>>();

    if usb_devices.is_empty() {
        warn!("No USB devices found.");
        return;
    }
}
