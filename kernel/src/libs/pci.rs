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
pub fn print_all_pci_devices() {
    println!("===== All PCI Devices =====");
    unsafe {
        for bus in 0..=255 {
            for slot in 0..32 {
                for func in 0..8 {
                    let location = PciLocation::new(bus, slot, func);
                    let identifier = location.pci_vendor_device();
                    let class = location.get_pci_class();
                    if identifier.vendor != 0xFFFF {
                        println!(
                            "Bus {:#02X}, Slot {:#02X}, Func {:#02X}: Vendor {:#04X}, Device {:#04X}, Class {:#02X}, Subclass {:#02X}, ProgIf {:#02X}", 
                            bus, slot, func,
                            identifier.vendor,
                            identifier.device, class.class,
                            class.subclass,
                            class.prog_if
                        );
                    }
                }
            }
        }
    }
    println!("===== End of PCI Bus Scan =====");
}

/// Scan all PCI devices and return a vector of (PciIdentifier, PciClass) tuples.
///
/// # Returns
///
/// - `Vec<(PciIdentifier, PciClass)>`: A vector of (PciIdentifier, PciClass) tuples.
pub fn scan_all_pci_devices() -> Vec<(PciIdentifier, PciClass)> {
    let mut devices = Vec::new();
    unsafe {
        for bus in 0..=255 {
            for slot in 0..32 {
                for func in 0..8 {
                    let location = PciLocation::new(bus, slot, func);
                    let identifier = location.pci_vendor_device();
                    let class = location.get_pci_class();
                    if identifier.vendor != 0xFFFF {
                        devices.push((identifier, class));
                    }
                }
            }
        }
    }
    devices
}
