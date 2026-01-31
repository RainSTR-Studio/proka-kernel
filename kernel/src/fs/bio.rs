extern crate alloc;
use crate::drivers::device::BlockDevice;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

pub struct BlockCache {
    device: Arc<dyn BlockDevice>,
    cache: RwLock<BTreeMap<usize, Vec<u8>>>,
}

impl BlockCache {
    pub fn new(device: Arc<dyn BlockDevice>) -> Self {
        Self {
            device,
            cache: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn read_block(&self, block_id: usize) -> Option<Vec<u8>> {
        {
            let cache = self.cache.read();
            if let Some(data) = cache.get(&block_id) {
                return Some(data.clone());
            }
        }

        let block_size = self.device.block_size();
        let mut buf = alloc::vec![0; block_size];

        if self.device.read_blocks(block_id, 1, &mut buf).is_ok() {
            let mut cache = self.cache.write();
            cache.insert(block_id, buf.clone());
            Some(buf)
        } else {
            None
        }
    }

    pub fn write_block(&self, block_id: usize, data: &[u8]) -> bool {
        if self.device.write_blocks(block_id, 1, data).is_ok() {
            let mut cache = self.cache.write();
            cache.insert(block_id, data.to_vec());
            true
        } else {
            false
        }
    }

    pub fn sync(&self) {}
}
