use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};

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
