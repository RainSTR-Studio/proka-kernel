use alloc::fmt::Debug;
use alloc::{format, string::String, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicBool, Ordering};
use spin::RwLock;

use crate::drivers::{bus::device_id::DeviceId, DeviceError, DeviceOps};

pub mod device_id;
pub mod tree;

/// Device tree node representing a hardware device
pub struct DeviceNode {
    /// Unique device identifier
    pub device_id: Arc<dyn DeviceId>,
    /// Human-readable device name
    pub name: String,
    /// Parent device (None for root devices)
    pub parent: Option<Arc<DeviceNode>>,
    /// Whether a driver is bound to this device
    pub driver_bound: AtomicBool,
    /// Raw device data for bus-specific use
    pub bus_data: Option<BusData>,
    /// The actual device operations, available after a driver is probed successfully.
    pub ops: RwLock<Option<DeviceOps>>,
}

impl Debug for DeviceNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DeviceNode")
            .field("device_id", &self.device_id)
            .field("name", &self.name)
            .field("parent", &self.parent)
            .field("driver_bound", &self.driver_bound)
            .field("bus_data", &self.bus_data)
            .finish()
    }
}

/// Bus-specific device data
#[derive(Debug)]
pub enum BusData {
    /// PCI device data
    Pci { bus: u8, slot: u8, func: u8 },
    /// Platform device data
    Platform { resources: Vec<(String, String)> },
}

impl DeviceNode {
    /// Create a new device node
    pub fn new(device_id: Arc<dyn DeviceId>, name: String) -> Self {
        Self {
            device_id,
            name,
            parent: None,
            driver_bound: AtomicBool::new(false),
            bus_data: None,
            ops: RwLock::new(None),
        }
    }

    /// Attach the device operations to this node after successful probing.
    pub fn attach_ops(&self, ops: DeviceOps) {
        *self.ops.write() = Some(ops);
    }

    /// Set the parent of this device
    pub fn set_parent(&mut self, parent: Arc<DeviceNode>) {
        self.parent = Some(parent);
    }

    /// Check if a driver is bound to this device
    pub fn is_bound(&self) -> bool {
        self.driver_bound.load(Ordering::SeqCst)
    }

    /// Mark this device as having a driver bound
    pub fn mark_bound(&self) {
        self.driver_bound.store(true, Ordering::SeqCst);
    }

    /// Mark this device as unbound (driver removed)
    pub fn mark_unbound(&self) {
        self.driver_bound.store(false, Ordering::SeqCst);
    }

    /// Get the device path (for VFS integration)
    pub fn get_path(&self) -> String {
        if let Some(ref parent) = self.parent {
            format!("{}/{}", parent.get_path(), self.name)
        } else {
            format!("/{}", self.name)
        }
    }
}

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    fn probe(&self, device: Arc<DeviceNode>) -> Result<(), DeviceError>;
    fn remove(&self, device: Arc<DeviceNode>) -> Result<(), DeviceError>;
}

pub trait Bus: Send + Sync {
    fn name(&self) -> &str;

    // 扫描总线返回所有设备
    fn scan(&self) -> Result<Vec<Arc<DeviceNode>>, DeviceError>;

    // 检查设备是否匹配驱动
    fn match_device(&self, device: &DeviceNode, driver: &dyn Driver) -> bool;

    // 注册驱动
    fn register_driver(&mut self, driver: Arc<dyn Driver>) -> Result<(), DeviceError>;

    // 移除驱动
    fn unregister_driver(&mut self, driver_name: &str) -> Result<(), DeviceError>;

    // 探测并绑定设备
    fn probe_devices(&mut self) -> Result<usize, DeviceError>;
}
