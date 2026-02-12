use crate::drivers::{BlockDevice, DeviceError, DeviceType, SharedDeviceOps};
extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

#[allow(dead_code)]
pub struct RamBlockDevice {
    name: String,
    storage: RwLock<Vec<u8>>,
    block_size: usize,
}

impl RamBlockDevice {
    #[allow(dead_code)]
    pub fn new(num_blocks: usize, block_size: usize) -> Self {
        Self {
            name: "ramdisk".to_string(),
            storage: RwLock::new(vec![0; num_blocks * block_size]),
            block_size,
        }
    }
}

impl SharedDeviceOps for RamBlockDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Block
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

impl BlockDevice for RamBlockDevice {
    fn block_size(&self) -> usize {
        self.block_size
    }

    fn num_blocks(&self) -> usize {
        self.storage.read().len() / self.block_size
    }

    fn read_blocks(
        &self,
        block_idx: usize,
        num_blocks: usize,
        buf: &mut [u8],
    ) -> Result<usize, DeviceError> {
        let start = block_idx * self.block_size;
        let end = start + num_blocks * self.block_size;
        let storage = self.storage.read();

        if end > storage.len() {
            return Err(DeviceError::AddressOutOfRange);
        }

        if buf.len() < end - start {
            return Err(DeviceError::BufferTooSmall);
        }

        buf[..end - start].copy_from_slice(&storage[start..end]);
        Ok(num_blocks)
    }

    fn write_blocks(
        &self,
        block_idx: usize,
        num_blocks: usize,
        buf: &[u8],
    ) -> Result<usize, DeviceError> {
        let start = block_idx * self.block_size;
        let end = start + num_blocks * self.block_size;
        let mut storage = self.storage.write();

        if end > storage.len() {
            return Err(DeviceError::AddressOutOfRange);
        }

        if buf.len() < end - start {
            return Err(DeviceError::InvalidParam);
        }

        storage[start..end].copy_from_slice(&buf[..end - start]);
        Ok(num_blocks)
    }
}
