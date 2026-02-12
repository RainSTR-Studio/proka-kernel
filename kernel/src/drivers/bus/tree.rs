//! # Global Device Tree
//!
//! This module defines the global `DeviceTree` which acts as a central repository
//! for all discovered `DeviceNode`s in the system.

use super::DeviceNode;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::RwLock;

/// The central device tree for the kernel.
pub struct DeviceTree {
    /// A flat list of all devices in the system for now.
    /// A proper tree structure with parent-child links can be built on top of this.
    devices: Vec<Arc<DeviceNode>>,
}

impl DeviceTree {
    /// Creates a new, empty device tree.
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Adds a device to the tree.
    pub fn add_device(&mut self, device: Arc<DeviceNode>) {
        self.devices.push(device);
    }

    /// Returns a list of all devices in the tree.
    pub fn list_devices(&self) -> &Vec<Arc<DeviceNode>> {
        &self.devices
    }
}

impl Default for DeviceTree {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static! {
    /// The global instance of the device tree.
    pub static ref DEVICE_TREE: RwLock<DeviceTree> = RwLock::new(DeviceTree::new());
}
