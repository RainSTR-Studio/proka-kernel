use log::debug;
use spin::Mutex;
use x86_64::instructions::port::Port;

/// PCI device structure with core identifiers and MMIO info
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PciDevice {
    // Bus:Device.Function address
    pub bus: u8,
    pub dev: u8,
    pub func: u8,

    // Device hardware identifiers
    pub vendor_id: u16,
    pub device_id: u16,
    pub revision_id: u8,
    pub prog_if: u8,

    // Device class code for driver matching
    pub class: u8,
    pub subclass: u8,

    // MMIO region from the first valid BAR
    pub mmio_base: u64,
    pub mmio_size: u64,
}

impl PciDevice {
    /// Create a zero-initialized PciDevice
    pub const fn zero() -> Self {
        Self {
            bus: 0,
            dev: 0,
            func: 0,
            vendor_id: 0,
            device_id: 0,
            revision_id: 0,
            prog_if: 0,
            class: 0,
            subclass: 0,
            mmio_base: 0,
            mmio_size: 0,
        }
    }
}

/// The PCI devices table.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PciDeviceTable {
    /// Each entries of device
    pub entries: [PciDevice; 64],

    /// Valid counts
    pub count: u8,
}

impl PciDeviceTable {
    pub const fn default() -> Self {
        Self {
            entries: [PciDevice::zero(); 64],
            count: 0,
        }
    }
}

/// Global PCI table
pub static PCI_DEVICES: Mutex<PciDeviceTable> = Mutex::new(PciDeviceTable::default());

// PCI configuration access ports
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Read a 32-bit value from PCI configuration space
pub fn pci_read(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    let addr = 0x80000000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        Port::new(PCI_CONFIG_ADDR).write(addr);
        Port::new(PCI_CONFIG_DATA).read()
    }
}

/// Write a 32-bit value to PCI configuration space
pub fn pci_write(bus: u8, dev: u8, func: u8, offset: u8, value: u32) {
    let addr = 0x80000000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        Port::new(PCI_CONFIG_ADDR).write(addr);
        Port::new(PCI_CONFIG_DATA).write(value);
    }
}

/// Read BAR and return (base, size), supporting 32-bit and 64-bit MMIO
fn pci_read_bar(bus: u8, dev: u8, func: u8, bar_idx: u8) -> Option<(u64, u64)> {
    let offset = 0x10 + bar_idx * 4;
    let orig = pci_read(bus, dev, func, offset);

    // Skip I/O port BARs
    if (orig & 1) != 0 {
        return None;
    }

    let is_64bit = ((orig >> 1) & 0b11) == 0b10;

    // Determine BAR size
    pci_write(bus, dev, func, offset, 0xFFFFFFFF);
    let mask = pci_read(bus, dev, func, offset);
    pci_write(bus, dev, func, offset, orig);

    let mut base = (orig & !0xF) as u64;

    let masked = mask & !0xF;
    if masked == 0 {
        return None;
    }
    let size = (!(masked) as u64).wrapping_add(1);

    // Handle 64-bit BAR
    if is_64bit {
        let high = pci_read(bus, dev, func, offset + 4);
        base |= (high as u64) << 32;
    }

    Some((base, size))
}

/// Scan PCI bus and populate the global PCI device list
pub fn pci_scan() {
    let mut pci_table = PCI_DEVICES.lock();

    for bus in 0..=255 {
        for dev in 0..=31 {
            for func in 0..=7 {
                let vend = pci_read(bus, dev, func, 0x00);
                if vend == 0xFFFFFFFF {
                    continue;
                }

                let class_rev = pci_read(bus, dev, func, 0x08);

                let mut dev_info = PciDevice {
                    bus,
                    dev,
                    func,
                    vendor_id: vend as u16,
                    device_id: (vend >> 16) as u16,
                    revision_id: class_rev as u8,
                    prog_if: (class_rev >> 8) as u8,
                    subclass: (class_rev >> 16) as u8,
                    class: (class_rev >> 24) as u8,
                    mmio_base: 0,
                    mmio_size: 0,
                };

                // Read first valid MMIO BAR
                for bar_idx in 0..6 {
                    if let Some((base, size)) = pci_read_bar(bus, dev, func, bar_idx) {
                        dev_info.mmio_base = base;
                        dev_info.mmio_size = size;
                        break;
                    }
                }

                let idx = pci_table.count;
                if idx >= 64 {
                    return;
                }

                pci_table.entries[idx as usize] = dev_info;
                pci_table.count = idx + 1;

                debug!(
                    "Got PCI device: bus {:02x}, device {:02x}, func {:02x}, vendor {:04x}, device {:04x}, class {:02x}/{:02x}, mmio {:016x}, size {:04x}",
                    dev_info.bus,
                    dev_info.dev,
                    dev_info.func,
                    dev_info.vendor_id,
                    dev_info.device_id,
                    dev_info.class,
                    dev_info.subclass,
                    dev_info.mmio_base,
                    dev_info.mmio_size
                );
            }
        }
    }
}

/// Iterate over all scanned PCI devices
pub fn pci_for_each<F: Fn(&PciDevice)>(f: F) {
    let pci_table = PCI_DEVICES.lock();
    for i in 0..pci_table.count as usize {
        f(&pci_table.entries[i]);
    }
}
