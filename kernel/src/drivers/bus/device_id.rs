use alloc::boxed::Box;
use alloc::fmt;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;

/// Trait for device identifiers
/// This allows different bus types to define their own ID schemes
pub trait DeviceId: fmt::Debug + Send + Sync {
    /// Return a string representation of this device ID
    fn name(&self) -> &str;

    /// Return a unique identifier string for this device
    fn unique_id(&self) -> String;

    /// Check if this device ID matches another
    fn matches(&self, other: &dyn DeviceId) -> bool {
        self.unique_id() == other.unique_id()
    }

    /// Convert this DeviceId into an Arc<dyn DeviceId>
    fn into_arc(self) -> Arc<dyn DeviceId>
    where
        Self: Sized + 'static;
}

impl fmt::Display for dyn DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.unique_id())
    }
}

/// PCI device identifier (Vendor ID + Device ID)
#[derive(Debug, Clone, Copy)]
pub struct PciDeviceId {
    /// PCI Vendor ID
    pub vendor: u16,
    /// PCI Device ID
    pub device: u16,
}

impl PartialEq for PciDeviceId {
    fn eq(&self, other: &Self) -> bool {
        self.vendor == other.vendor && self.device == other.device
    }
}

impl Eq for PciDeviceId {}

impl PciDeviceId {
    /// Create a new PCI device ID
    pub const fn new(vendor: u16, device: u16) -> Self {
        Self { vendor, device }
    }
}

impl DeviceId for PciDeviceId {
    fn name(&self) -> &str {
        "pci"
    }

    fn unique_id(&self) -> String {
        format!("pci_{:#06x}_{:#06x}", self.vendor, self.device)
    }

    fn into_arc(self) -> Arc<dyn DeviceId> {
        Arc::from(Box::new(self) as Box<dyn DeviceId>)
    }
}

/// Platform device identifier (name-based)
/// Used for legacy hardware without PCI/compatible identifiers
#[derive(Debug, Clone)]
pub struct PlatformDeviceId {
    /// Device name (e.g., "serial", "keyboard", "rtc")
    pub name: String,
}

impl PartialEq for PlatformDeviceId {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for PlatformDeviceId {}

impl PlatformDeviceId {
    /// Create a new platform device ID
    pub fn new<T: Into<String>>(name: T) -> Self {
        Self { name: name.into() }
    }
}

impl DeviceId for PlatformDeviceId {
    fn name(&self) -> &str {
        "platform"
    }

    fn unique_id(&self) -> String {
        format!("platform_{}", self.name)
    }

    fn into_arc(self) -> Arc<dyn DeviceId> {
        Arc::from(Box::new(self) as Box<dyn DeviceId>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_pci_device_id() {
        let id = PciDeviceId::new(0x8086, 0x1234);
        assert_eq!(id.vendor, 0x8086);
        assert_eq!(id.device, 0x1234);
        assert_eq!(id.name(), "pci");
        assert_eq!(id.unique_id(), "pci_0x8086_0x1234");
    }

    #[test_case]
    fn test_platform_device_id() {
        let id = PlatformDeviceId::new("serial");
        assert_eq!(id.name, "serial");
        assert_eq!(id.name(), "platform");
        assert_eq!(id.unique_id(), "platform_serial");
    }

    #[test_case]
    fn test_device_id_matches() {
        let pci = PciDeviceId::new(0x1234, 0x5678);
        let plat = PlatformDeviceId::new("keyboard");

        let pci_arc: Arc<dyn DeviceId> = pci.into_arc();
        let plat_arc: Arc<dyn DeviceId> = plat.into_arc();

        assert_eq!(pci_arc.name(), "pci");
        assert_eq!(plat_arc.name(), "platform");

        // Test matching
        assert!(!pci_arc.matches(plat_arc.as_ref()));
        assert!(pci_arc.matches(pci_arc.as_ref()));
    }
}
