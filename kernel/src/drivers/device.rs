extern crate alloc;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Block,
    Char,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DeviceError {
    InvalidParam,
    NotSupported,
    IoError,
    PermissionsDenied,
    NoSuchDevice,
    WouldBlock,
    Busy,
    OutOfMemory,
    DeviceClosed,
    BufferTooSmall,
    AlreadyOpen,
    NotOpen,
    AddressOutOfRange,
    DeviceAlreadyRegistered,
    DeviceNumberConflict,
    DeviceNotRegistered,
    DeviceStillInUse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanInfo {
    pub device_id: String,                                 // Device ID
    pub protocol_type: String, // Communication protocol type (e.g. USB/PCI/I2C)
    pub vendor_id: Option<u16>, // Vendor ID
    pub product_id: Option<u16>, // Product ID
    pub additional_data: Option<BTreeMap<String, String>>, // Additional data
}

pub trait SharedDeviceOps: Send + Sync {
    fn name(&self) -> &str;
    fn device_type(&self) -> DeviceType;

    fn open(&self) -> Result<(), DeviceError>;
    fn close(&self) -> Result<(), DeviceError>;
    fn ioctl(&self, cmd: u64, arg: u64) -> Result<u64, DeviceError>;

    fn sync(&self) -> Result<(), DeviceError> {
        Err(DeviceError::NotSupported)
    }
    fn is_compatible(&self, _scan_info: &ScanInfo) -> bool {
        false
    }
}

pub trait BlockDevice: SharedDeviceOps {
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> usize;

    fn read_blocks(
        &self,
        block_idx: usize,
        num_blocks: usize,
        buf: &mut [u8],
    ) -> Result<usize, DeviceError>;

    fn write_blocks(
        &self,
        block_idx: usize,
        num_blocks: usize,
        buf: &[u8],
    ) -> Result<usize, DeviceError>;

    fn erase_blocks(&self, start_block: usize, num_blocks: usize) -> Result<usize, DeviceError> {
        let _ = (start_block, num_blocks);
        Err(DeviceError::NotSupported)
    }
}

pub trait CharDevice: SharedDeviceOps {
    fn read(&self, buf: &mut [u8]) -> Result<usize, DeviceError>;
    fn write(&self, buf: &[u8]) -> Result<usize, DeviceError>;

    fn peek(&self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        let _ = buf;
        Err(DeviceError::NotSupported)
    }

    fn has_data(&self) -> bool {
        false
    }

    fn has_space(&self) -> bool {
        false
    }

    fn set_nonblocking(&self, nonblocking: bool) -> Result<(), DeviceError> {
        let _ = nonblocking;
        Err(DeviceError::NotSupported)
    }
}

#[derive(Clone)]
pub enum DeviceInner {
    Char(Arc<dyn CharDevice>),
    Block(Arc<dyn BlockDevice>),
}

#[cfg(test)]
pub struct TestDevice;

#[cfg(test)]
impl SharedDeviceOps for TestDevice {
    fn name(&self) -> &str {
        "null"
    }
    fn device_type(&self) -> DeviceType {
        DeviceType::Char
    }

    fn open(&self) -> Result<(), DeviceError> {
        Ok(())
    }
    fn close(&self) -> Result<(), DeviceError> {
        Ok(())
    }
    fn ioctl(&self, _cmd: u64, _arg: u64) -> Result<u64, DeviceError> {
        Err(DeviceError::NotSupported)
    }
}

#[cfg(test)]
impl CharDevice for TestDevice {
    fn read(&self, _buf: &mut [u8]) -> Result<usize, DeviceError> {
        Ok(0)
    }
    fn write(&self, buf: &[u8]) -> Result<usize, DeviceError> {
        Ok(buf.len())
    }
}

pub struct Device {
    pub name: String,
    pub major: u16,
    pub minor: u16,
    pub inner: DeviceInner,
    open_count: AtomicUsize,
    is_registered: AtomicBool,
}

impl Device {
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

    #[cfg(test)]
    pub fn null() -> Self {
        Self::new_auto_assign(
            String::from("null"),
            DeviceInner::Char(Arc::new(TestDevice)),
        )
    }

    #[inline]
    fn shared_ops(&self) -> &dyn SharedDeviceOps {
        match &self.inner {
            DeviceInner::Block(ops) => ops.as_ref(),
            DeviceInner::Char(ops) => ops.as_ref(),
        }
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

impl core::fmt::Debug for Device {
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

impl Clone for Device {
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
    devices: Vec<Arc<Device>>,
    next_minor_counters: BTreeMap<u16, u16>,
    free_minors: BTreeMap<u16, Vec<u16>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_device(&mut self, mut device: Device) -> Result<Arc<Device>, DeviceError> {
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

    pub fn get_device(&self, name: &str) -> Option<Arc<Device>> {
        self.devices.iter().find(|d| d.name == name).cloned()
    }

    pub fn get_device_by_major_minor(&self, major: u16, minor: u16) -> Option<Arc<Device>> {
        self.devices
            .iter()
            .find(|d| d.major == major && d.minor == minor)
            .cloned()
    }

    pub fn get_devices_by_type(&self, device_type: DeviceType) -> Vec<Arc<Device>> {
        self.devices
            .iter()
            .filter(|d| d.device_type() == device_type)
            .cloned()
            .collect()
    }

    pub fn list_devices(&self) -> Vec<Arc<Device>> {
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
