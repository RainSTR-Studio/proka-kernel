extern crate alloc;
use crate::println;
use alloc::vec::Vec;
use x86_64::instructions::port::Port;

const PCI_ADDR: u16 = 0xCF8;
const PCI_DATA: u16 = 0xCFC;

#[derive(Debug, Clone, Copy)]
pub struct PciLocation {
    /// The bus number of the PCI device.
    pub bus: u8,

    /// The slot number of the PCI device.
    pub slot: u8,

    /// The function number of the PCI device.
    pub func: u8,
}

impl PciLocation {
    /// Create a new PCI device.
    ///
    /// # Arguments
    ///
    /// - `bus`: The bus number of the PCI device.
    /// - `slot`: The slot number of the PCI device.
    /// - `func`: The function number of the PCI device.
    ///
    /// # Returns
    ///
    /// - `PciDevice`: The new PCI device.
    pub fn new(bus: u8, slot: u8, func: u8) -> Self {
        Self { bus, slot, func }
    }

    /// Read a 32-bit value from the PCI device's configuration space.
    ///
    /// # Arguments
    ///
    /// - `offset`: The offset within the configuration space to read from.
    ///
    /// # Returns
    ///
    /// - `u32`: The 32-bit value read from the configuration space.
    pub unsafe fn pci_read(&self, offset: u8) -> u32 {
        let mut addr_port = Port::<u32>::new(PCI_ADDR);
        let mut data_port = Port::<u32>::new(PCI_DATA);

        let addr = (1 << 31)
            | (self.bus as u32) << 16
            | (self.slot as u32) << 11
            | (self.func as u32) << 8
            | (offset & 0xFC) as u32;

        addr_port.write(addr);
        let data = data_port.read();

        data
    }

    /// Read the vendor and device ID of the PCI device.
    ///
    /// # Returns
    ///
    /// - (u16, u16): A tuple of (vendor, device)
    pub unsafe fn pci_vendor_device(&self) -> PciIdentifier {
        let val = self.pci_read(0x00);
        let vendor = (val >> 16) as u16;
        let device = (val & 0xFFFF) as u16;
        PciIdentifier { vendor, device }
    }

    /// Get the class, subclass, and prog_if of the PCI device.
    ///
    /// # Returns
    ///
    /// - (u8, u8, u8): A tuple of (class, subclass, prog_if)
    pub unsafe fn get_pci_class(&self) -> PciClass {
        let val = self.pci_read(0x08);
        let class = (val >> 16) as u8;
        let subclass = (val >> 8) as u8;
        let prog_if = (val & 0xFF) as u8;
        PciClass {
            class,
            subclass,
            prog_if,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PciIdentifier {
    /// The vendor ID of the PCI device.
    pub vendor: u16,

    /// The device ID of the PCI device.
    pub device: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct PciClass {
    /// The class of the PCI device.
    pub class: u8,

    /// The subclass of the PCI device.
    pub subclass: u8,

    /// The prog_if of the PCI device.
    pub prog_if: u8,
}

/// Print all PCI devices to console.
/// Uses optimized scanning logic similar to scan_all_pci_devices().
pub fn print_all_pci_devices() {
    println!("===== All PCI Devices =====");

    unsafe {
        let mut addr_port = Port::<u32>::new(PCI_ADDR);
        let mut data_port = Port::<u32>::new(PCI_DATA);

        for bus in 0..=255 {
            for slot in 0..32 {
                let _location = PciLocation::new(bus, slot, 0);

                let addr = (1u32 << 31) | (bus as u32) << 16 | (slot as u32) << 11;

                addr_port.write(addr);
                let vendor_data = data_port.read();
                let vendor = (vendor_data >> 16) as u16;

                if vendor == 0xFFFF {
                    continue;
                }

                let device = (vendor_data & 0xFFFF) as u16;

                // Read class info
                addr_port.write(addr | 0x08);
                let class_data = data_port.read();
                let class = (class_data >> 16) as u8;
                let subclass = (class_data >> 8) as u8;
                let prog_if = (class_data & 0xFF) as u8;

                println!(
                    "Bus {:#02X}, Slot {:#02X}, Func {:#02X}: Vendor {:#04X}, Device {:#04X}, Class {:#02X}, Subclass {:#02X}, ProgIf {:#02X}", 
                    bus, slot, 0,
                    vendor,
                    device, class,
                    subclass,
                    prog_if
                );

                // Check multi-function
                addr_port.write(addr | 0x0E);
                let header_data = data_port.read();
                let header_type = (header_data & 0xFF) as u8;

                let max_func = if (header_type & 0x80) != 0 { 7 } else { 0 };

                for func in 1..=max_func {
                    let func_addr = addr | ((func as u32) << 8);

                    addr_port.write(func_addr);
                    let func_vendor_data = data_port.read();
                    let func_vendor = (func_vendor_data >> 16) as u16;

                    if func_vendor == 0xFFFF {
                        continue;
                    }

                    let func_device = (func_vendor_data & 0xFFFF) as u16;

                    addr_port.write(func_addr | 0x08);
                    let func_class_data = data_port.read();
                    let func_class = (func_class_data >> 16) as u8;
                    let func_subclass = (func_class_data >> 8) as u8;
                    let func_prog_if = (func_class_data & 0xFF) as u8;

                    println!(
                        "Bus {:#02X}, Slot {:#02X}, Func {:#02X}: Vendor {:#04X}, Device {:#04X}, Class {:#02X}, Subclass {:#02X}, ProgIf {:#02X}", 
                        bus, slot, func,
                        func_vendor,
                        func_device, func_class,
                        func_subclass,
                        func_prog_if
                    );
                }
            }
        }
    }
    println!("===== End of PCI Bus Scan =====");
}

/// Scan all PCI devices and return a vector of (PciIdentifier, PciClass) tuples.
///
/// Optimized version with:
/// - Port object reuse (avoids recreating Port for each read)
/// - Early exit on invalid devices (check vendor_id first)
/// - Multi-function device detection (skip non-existent functions)
pub fn scan_all_pci_devices() -> Vec<(PciIdentifier, PciClass)> {
    let mut devices = Vec::new();

    unsafe {
        let mut addr_port = Port::<u32>::new(PCI_ADDR);
        let mut data_port = Port::<u32>::new(PCI_DATA);

        for bus in 0..=255 {
            for slot in 0..32 {
                // Check if any device exists at this slot (func 0)
                // by reading vendor_id first - early exit if no device
                let _location = PciLocation::new(bus, slot, 0);

                // Reuse port objects for this location
                let addr = (1u32 << 31) | (bus as u32) << 16 | (slot as u32) << 11 | 0u32; // func = 0

                addr_port.write(addr);
                let vendor_data = data_port.read();
                let vendor = (vendor_data >> 16) as u16;

                // No device at this slot - skip entire slot (all 8 functions)
                if vendor == 0xFFFF {
                    continue;
                }

                // Device exists - get full info
                let device = (vendor_data & 0xFFFF) as u16;
                let identifier = PciIdentifier { vendor, device };

                // Read class info (offset 0x08)
                let class_addr = addr | 0x08;
                addr_port.write(class_addr);
                let class_data = data_port.read();
                let class = (class_data >> 16) as u8;
                let subclass = (class_data >> 8) as u8;
                let prog_if = (class_data & 0xFF) as u8;
                let pci_class = PciClass {
                    class,
                    subclass,
                    prog_if,
                };

                devices.push((identifier, pci_class));

                // Check if this is a multi-function device
                // Read header type at offset 0x0E (only lower byte matters)
                let header_addr = addr | 0x0E;
                addr_port.write(header_addr);
                let header_data = data_port.read();
                let header_type = (header_data & 0xFF) as u8;

                // If bit 7 set, it's a multi-function device - scan all functions
                // Otherwise only func 0 exists
                let max_func = if (header_type & 0x80) != 0 { 7 } else { 0 };

                for func in 1..=max_func {
                    let func_addr = (1u32 << 31)
                        | (bus as u32) << 16
                        | (slot as u32) << 11
                        | (func as u32) << 8;

                    addr_port.write(func_addr);
                    let func_vendor_data = data_port.read();
                    let func_vendor = (func_vendor_data >> 16) as u16;

                    if func_vendor == 0xFFFF {
                        continue;
                    }

                    let func_device = (func_vendor_data & 0xFFFF) as u16;
                    let func_identifier = PciIdentifier {
                        vendor: func_vendor,
                        device: func_device,
                    };

                    // Read class info for this function
                    addr_port.write(func_addr | 0x08);
                    let func_class_data = data_port.read();
                    let func_class = (func_class_data >> 16) as u8;
                    let func_subclass = (func_class_data >> 8) as u8;
                    let func_prog_if = (func_class_data & 0xFF) as u8;
                    let func_pci_class = PciClass {
                        class: func_class,
                        subclass: func_subclass,
                        prog_if: func_prog_if,
                    };

                    devices.push((func_identifier, func_pci_class));
                }
            }
        }
    }

    devices
}
