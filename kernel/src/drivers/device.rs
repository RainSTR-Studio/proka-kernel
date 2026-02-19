extern crate alloc;
use super::{BlockDevice, CharDevice, DeviceError, DeviceInner, DeviceType, SharedDeviceOps};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use lazy_static::lazy_static;
use spin::RwLock;

lazy_static! {
    pub static ref DEVICE_MANAGER: RwLock<DeviceManager> = RwLock::new(DeviceManager::new());
}

pub struct OldDevice {
    pub name: String,
    pub major: u16,
    pub minor: u16,
    pub inner: DeviceInner,
    open_count: AtomicUsize,
    is_registered: AtomicBool,
}

impl OldDevice {
    pub fn new(name: String, major: u16, minor: u16, inner: DeviceInner) -> Self {
        Self {
            name,
            major,
            minor,
            inner,
            open_count: AtomicUsize::new(0),
            is_registered: AtomicBool::new(false),
        }
    }

    pub fn new_auto_assign(name: String, inner: DeviceInner) -> Self {
        Self {
            name,
            major: 0,
            minor: 0,
            inner,
            open_count: AtomicUsize::new(0),
            is_registered: AtomicBool::new(false),
        }
    }

    #[inline]
    fn shared_ops(&self) -> &dyn SharedDeviceOps {
        match &self.inner {
            DeviceInner::Block(ops) => ops.as_ref(),
            DeviceInner::Char(ops) => ops.as_ref(),
        }
    }

    #[cfg(test)]
    pub fn null() -> Self {
        Self::new_auto_assign(
            String::from("null"),
            DeviceInner::Char(Arc::new(crate::drivers::TestDevice)),
        )
    }

    pub fn device_type(&self) -> DeviceType {
        self.shared_ops().device_type()
    }

    pub fn open(&self) -> Result<(), DeviceError> {
        if !self.is_registered.load(Ordering::SeqCst) {
            return Err(DeviceError::DeviceNotRegistered);
        }

        let current_count = self.open_count.fetch_add(1, Ordering::SeqCst);
        if current_count == 0 {
            self.shared_ops().open()?;
        }
        Ok(())
    }

    pub fn close(&self) -> Result<(), DeviceError> {
        if !self.is_registered.load(Ordering::SeqCst) {
            return Err(DeviceError::DeviceNotRegistered);
        }

        let current_count = self.open_count.fetch_sub(1, Ordering::SeqCst);
        if current_count == 1 {
            self.shared_ops().close()?;
        } else if current_count == 0 {
            return Err(DeviceError::NotOpen);
        }
        Ok(())
    }

    pub fn ioctl(&self, cmd: u64, arg: u64) -> Result<u64, DeviceError> {
        if !self.is_registered.load(Ordering::SeqCst) {
            return Err(DeviceError::DeviceNotRegistered);
        }
        if self.open_count.load(Ordering::SeqCst) == 0 {
            return Err(DeviceError::DeviceClosed);
        }
        self.shared_ops().ioctl(cmd, arg)
    }

    pub fn is_open(&self) -> bool {
        self.open_count.load(Ordering::Relaxed) > 0
    }

    pub fn as_block_device(&self) -> Option<&Arc<dyn BlockDevice>> {
        if let DeviceInner::Block(ref ops) = self.inner {
            Some(ops)
        } else {
            None
        }
    }

    pub fn as_char_device(&self) -> Option<&Arc<dyn CharDevice>> {
        if let DeviceInner::Char(ref ops) = self.inner {
            Some(ops)
        } else {
            None
        }
    }

    fn mark_registered(&self) {
        self.is_registered.store(true, Ordering::SeqCst);
    }

    fn mark_unregistered(&self) {
        self.is_registered.store(false, Ordering::SeqCst);
    }
}

impl core::fmt::Debug for OldDevice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Device {{ name: {}, major: {}, minor: {}, open_count: {}, is_registered: {} }}",
            self.name,
            self.major,
            self.minor,
            self.open_count.load(Ordering::SeqCst),
            self.is_registered.load(Ordering::SeqCst)
        )
    }
}

impl Clone for OldDevice {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            major: self.major,
            minor: self.minor,
            open_count: AtomicUsize::new(self.open_count.load(Ordering::SeqCst)),
            is_registered: AtomicBool::new(self.is_registered.load(Ordering::SeqCst)),
            inner: self.inner.clone(),
        }
    }
}

#[derive(Default)]
pub struct DeviceManager {
    devices: Vec<Arc<OldDevice>>,
    next_minor_counters: BTreeMap<u16, u16>,
    free_minors: BTreeMap<u16, Vec<u16>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_device(
        &mut self,
        mut device: OldDevice,
    ) -> Result<Arc<OldDevice>, DeviceError> {
        if self.devices.iter().any(|d| d.name == device.name) {
            return Err(DeviceError::DeviceAlreadyRegistered);
        }

        if device.major == 0 && device.minor == 0 {
            let (major, minor) = self.alloc_device_number(device.device_type())?;
            device.major = major;
            device.minor = minor;
        } else {
            if self
                .devices
                .iter()
                .any(|d| d.major == device.major && d.minor == device.minor)
            {
                return Err(DeviceError::DeviceNumberConflict);
            }

            self.update_minor_counter(device.major, device.minor);
        }

        device.mark_registered();
        let device_arc = Arc::new(device);
        self.devices.push(device_arc.clone());
        Ok(device_arc)
    }

    fn alloc_device_number(&mut self, device_type: DeviceType) -> Result<(u16, u16), DeviceError> {
        let major = match device_type {
            DeviceType::Char => 1,
            DeviceType::Block => 2,
        };

        if let Some(minor) = self.free_minors.get_mut(&major).and_then(|v| v.pop()) {
            return Ok((major, minor));
        }

        let next_minor = self.next_minor_counters.entry(major).or_insert(0);
        let mut current_minor = *next_minor;

        for _ in 0..u16::MAX as usize {
            let is_used = self
                .devices
                .iter()
                .any(|d| d.major == major && d.minor == current_minor);

            if !is_used {
                *next_minor = current_minor.checked_add(1).unwrap_or(0);
                return Ok((major, current_minor));
            }
            current_minor = current_minor.checked_add(1).unwrap_or(0);
        }

        Err(DeviceError::OutOfMemory)
    }

    pub fn is_minor_used(&self, major: u16, minor: u16) -> bool {
        self.devices
            .iter()
            .any(|d| d.major == major && d.minor == minor)
    }

    fn update_minor_counter(&mut self, major: u16, minor: u16) {
        let counter = self.next_minor_counters.entry(major).or_insert(0);
        if minor >= *counter {
            *counter = minor + 1;
        }
    }

    pub fn unregister_device(&mut self, name: &str) -> Result<(), DeviceError> {
        let position = self.devices.iter().position(|d| d.name == name);

        if let Some(index) = position {
            if self.devices[index].is_open() {
                return Err(DeviceError::DeviceStillInUse);
            }

            let device_arc = self.devices.remove(index);
            device_arc.mark_unregistered();
            self.reclaim_device_number(device_arc.major, device_arc.minor);
            Ok(())
        } else {
            Err(DeviceError::NoSuchDevice)
        }
    }

    fn reclaim_device_number(&mut self, major: u16, minor: u16) {
        self.free_minors.entry(major).or_default().push(minor);
    }

    pub fn get_device(&self, name: &str) -> Option<Arc<OldDevice>> {
        self.devices.iter().find(|d| d.name == name).cloned()
    }

    pub fn get_device_by_major_minor(&self, major: u16, minor: u16) -> Option<Arc<OldDevice>> {
        self.devices
            .iter()
            .find(|d| d.major == major && d.minor == minor)
            .cloned()
    }

    pub fn get_devices_by_type(&self, device_type: DeviceType) -> Vec<Arc<OldDevice>> {
        self.devices
            .iter()
            .filter(|d| d.device_type() == device_type)
            .cloned()
            .collect()
    }

    pub fn list_devices(&self) -> Vec<Arc<OldDevice>> {
        self.devices.clone()
    }
}

pub fn init_devices() {
    let mut manager = DEVICE_MANAGER.write();
    manager
        .register_device(super::char::serial::SerialDevice::create_device(
            1, 0, 0x3f8,
        ))
        .expect("Failed to register serial device");

    manager
        .register_device(super::input::ps2::keyboard::Keyboard::create_device())
        .expect("Failed to register keyboard device");
}
