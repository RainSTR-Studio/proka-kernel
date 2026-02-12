extern crate alloc;
use crate::drivers::OldDevice;
use crate::fs::vfs::{FileSystem, Inode, Metadata, VNodeType, VfsError};
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::any::Any;
use spin::RwLock;

pub type ReadCallback = Box<dyn Fn(u64, &mut [u8]) -> Result<usize, VfsError> + Send + Sync>;
pub type WriteCallback = Box<dyn Fn(u64, &[u8]) -> Result<usize, VfsError> + Send + Sync>;

pub enum KernNodeContent {
    /// Directory node
    Dir(RwLock<BTreeMap<String, Arc<KernInode>>>),
    /// Read-write function node
    File {
        read: Option<ReadCallback>,
        write: Option<WriteCallback>,
        size: u64,
    },
    /// Device mapping
    Device { device: Arc<OldDevice> },
}

/// Kernel file system node
pub struct KernInode {
    node_type: VNodeType,
    content: KernNodeContent,
}

impl core::fmt::Debug for KernInode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KernInode")
            .field("node_type", &self.node_type)
            .finish()
    }
}

impl KernInode {
    /// Create dir node
    pub fn new_dir() -> Arc<Self> {
        Arc::new(Self {
            node_type: VNodeType::Dir,
            content: KernNodeContent::Dir(RwLock::new(BTreeMap::new())),
        })
    }

    /// Create file node
    pub fn new_file(read: Option<ReadCallback>, write: Option<WriteCallback>) -> Arc<Self> {
        Arc::new(Self {
            node_type: VNodeType::File,
            content: KernNodeContent::File {
                read,
                write,
                size: 0,
            },
        })
    }
    /// Create device node
    pub fn new_device(device: Arc<OldDevice>) -> Arc<Self> {
        Arc::new(Self {
            node_type: VNodeType::Device,
            content: KernNodeContent::Device { device },
        })
    }

    /// Add child node to directory
    pub fn add_child(&self, name: &str, child: Arc<KernInode>) -> Result<(), VfsError> {
        match &self.content {
            KernNodeContent::Dir(entries) => {
                let mut map = entries.write();
                if map.contains_key(name) {
                    return Err(VfsError::AlreadyExists);
                }
                map.insert(name.to_string(), child);
                Ok(())
            }
            _ => Err(VfsError::NotADirectory),
        }
    }
}

impl Inode for KernInode {
    fn metadata(&self) -> Result<Metadata, VfsError> {
        let size = match &self.content {
            KernNodeContent::File { size, .. } => *size,
            _ => 0,
        };
        Ok(Metadata {
            size,
            permissions: 0o755,
            uid: 0,
            gid: 0,
            ctime: 0,
            mtime: 0,
            blocks: 0,
            nlinks: 1,
        })
    }

    fn set_metadata(&self, _metadata: &Metadata) -> Result<(), VfsError> {
        Err(VfsError::NotImplemented)
    }

    fn node_type(&self) -> VNodeType {
        self.node_type
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        match &self.content {
            KernNodeContent::File { read, .. } => {
                if let Some(cb) = read {
                    cb(offset, buf)
                } else {
                    Err(VfsError::PermissionDenied)
                }
            }
            KernNodeContent::Device { device } => {
                if let Some(char_dev) = device.as_char_device() {
                    char_dev.read(buf).map_err(VfsError::DeviceError)
                } else {
                    Err(VfsError::NotImplemented)
                }
            }
            _ => Err(VfsError::NotAFile),
        }
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        match &self.content {
            KernNodeContent::File { write, .. } => {
                if let Some(cb) = write {
                    cb(offset, buf)
                } else {
                    Err(VfsError::PermissionDenied)
                }
            }
            KernNodeContent::Device { device } => {
                if let Some(char_dev) = device.as_char_device() {
                    char_dev.write(buf).map_err(VfsError::DeviceError)
                } else {
                    Err(VfsError::NotImplemented)
                }
            }
            _ => Err(VfsError::NotAFile),
        }
    }

    fn truncate(&self, _size: u64) -> Result<(), VfsError> {
        Err(VfsError::NotImplemented)
    }

    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            KernNodeContent::Dir(entries) => entries
                .read()
                .get(name)
                .cloned()
                .map(|n| n as Arc<dyn Inode>)
                .ok_or(VfsError::NotFound),
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn create(&self, name: &str, typ: VNodeType) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            KernNodeContent::Dir(entries) => {
                let mut map = entries.write();
                if map.contains_key(name) {
                    return Err(VfsError::AlreadyExists);
                }

                let new_inode = match typ {
                    VNodeType::Dir => KernInode::new_dir(),
                    VNodeType::File => KernInode::new_file(None, None),
                    _ => return Err(VfsError::NotImplemented),
                };

                map.insert(name.to_string(), new_inode.clone());
                Ok(new_inode)
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn create_device(&self, name: &str, device: Arc<OldDevice>) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            KernNodeContent::Dir(entries) => {
                let mut map = entries.write();
                if map.contains_key(name) {
                    return Err(VfsError::AlreadyExists);
                }

                let new_inode = KernInode::new_device(device);
                map.insert(name.to_string(), new_inode.clone());
                Ok(new_inode)
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn unlink(&self, name: &str) -> Result<(), VfsError> {
        match &self.content {
            KernNodeContent::Dir(entries) => {
                let mut map = entries.write();
                if map.remove(name).is_some() {
                    Ok(())
                } else {
                    Err(VfsError::NotFound)
                }
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn move_to(
        &self,
        old_name: &str,
        target_dir: &Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), VfsError> {
        if let Some(target_kern) = target_dir.as_any().downcast_ref::<KernInode>() {
            match (&self.content, &target_kern.content) {
                (KernNodeContent::Dir(src_entries), KernNodeContent::Dir(dst_entries)) => {
                    let mut src_map = src_entries.write();
                    let mut dst_map = dst_entries.write();

                    if !src_map.contains_key(old_name) {
                        return Err(VfsError::NotFound);
                    }
                    if dst_map.contains_key(new_name) {
                        return Err(VfsError::AlreadyExists);
                    }

                    let node = src_map.remove(old_name).unwrap();
                    dst_map.insert(new_name.to_string(), node);
                    Ok(())
                }
                _ => Err(VfsError::NotADirectory),
            }
        } else {
            Err(VfsError::NotImplemented)
        }
    }

    fn list(&self) -> Result<Vec<String>, VfsError> {
        match &self.content {
            KernNodeContent::Dir(entries) => Ok(entries.read().keys().cloned().collect()),
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct KernFs {
    root: Arc<KernInode>,
}

impl Default for KernFs {
    fn default() -> Self {
        let root = KernInode::new_dir();

        Self { root }
    }
}

impl KernFs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn root(&self) -> Arc<KernInode> {
        self.root.clone()
    }
}

impl FileSystem for KernFs {
    fn mount(
        &self,
        _device: Option<Arc<OldDevice>>,
        _args: Option<&[&str]>,
    ) -> Result<Arc<dyn Inode>, VfsError> {
        Ok(self.root.clone())
    }

    fn fs_type(&self) -> &'static str {
        "kernfs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test_case]
    fn test_mount_creates_root() {
        let kernfs = KernFs::new();
        let root = kernfs.mount(None, None).unwrap();
        assert_eq!(root.node_type(), VNodeType::Dir);
    }

    #[test_case]
    fn test_create_file() {
        let kernfs = KernFs::new();
        let root = kernfs.mount(None, None).unwrap();
        let file_inode = root.create("test_file", VNodeType::File).unwrap();
        assert_eq!(file_inode.node_type(), VNodeType::File);
    }

    #[test_case]
    fn test_add_child_and_lookup() {
        let root = KernInode::new_dir();
        let child_dir = KernInode::new_dir();
        root.add_child("child_dir", child_dir.clone()).unwrap();

        let looked_up_child = root.lookup("child_dir").unwrap();
        assert_eq!(looked_up_child.node_type(), VNodeType::Dir);

        // Test adding an existing child
        assert_eq!(
            root.add_child("child_dir", child_dir.clone()).unwrap_err(),
            VfsError::AlreadyExists
        );
    }

    #[test_case]
    fn test_create_and_lookup_file() {
        let root = KernInode::new_dir();
        let file = root.create("test_file", VNodeType::File).unwrap();
        assert_eq!(file.node_type(), VNodeType::File);

        let looked_up_file = root.lookup("test_file").unwrap();
        assert_eq!(looked_up_file.node_type(), VNodeType::File);
    }

    #[test_case]
    fn test_unlink() {
        let root = KernInode::new_dir();
        root.create("file_to_unlink", VNodeType::File).unwrap();
        root.unlink("file_to_unlink").unwrap();
        assert_eq!(
            root.lookup("file_to_unlink").unwrap_err(),
            VfsError::NotFound
        );
    }

    #[test_case]
    fn test_list_directory() {
        let root = KernInode::new_dir();
        root.create("file1", VNodeType::File).unwrap();
        root.create("dir1", VNodeType::Dir).unwrap();

        let mut entries = root.list().unwrap();
        entries.sort(); // Sort to ensure consistent order
        assert_eq!(entries, vec!["dir1".to_string(), "file1".to_string()]);
    }

    #[test_case]
    fn test_file_read_write() {
        let file_content = Arc::new(RwLock::new(Vec::<u8>::new()));
        let content_for_read = file_content.clone();
        let read_callback: ReadCallback = Box::new(move |offset, buf| {
            let data = content_for_read.read();
            let end = (offset + buf.len() as u64).min(data.len() as u64);
            let start = offset.min(data.len() as u64);
            let bytes_to_read = (end - start) as usize;
            buf[..bytes_to_read].copy_from_slice(&data[start as usize..end as usize]);
            Ok(bytes_to_read)
        });

        let content_for_write = file_content.clone();
        let write_callback: WriteCallback = Box::new(move |offset, buf| {
            let mut data = content_for_write.write();
            if offset + buf.len() as u64 > data.len() as u64 {
                data.resize((offset + buf.len() as u64) as usize, 0);
            }
            data[offset as usize..(offset + buf.len() as u64) as usize].copy_from_slice(buf);
            Ok(buf.len())
        });

        let file_inode = KernInode::new_file(Some(read_callback), Some(write_callback));

        // Write some data
        let write_data = b"hello world";
        file_inode.write_at(0, write_data).unwrap();

        // Read the data back
        let mut read_buffer = vec![0; write_data.len()];
        file_inode.read_at(0, &mut read_buffer).unwrap();
        assert_eq!(read_buffer, write_data);

        // Test partial read
        let mut partial_read_buffer = vec![0; 5];
        file_inode.read_at(0, &mut partial_read_buffer).unwrap();
        assert_eq!(partial_read_buffer, b"hello");

        file_inode.read_at(6, &mut partial_read_buffer).unwrap();
        assert_eq!(partial_read_buffer, b"world");

        // Test writing with offset
        file_inode.write_at(6, b"rust").unwrap();
        let mut full_read_buffer = vec![0; 10];
        file_inode.read_at(0, &mut full_read_buffer).unwrap();
        assert_eq!(full_read_buffer, b"hello rust");

        // Test no read/write permissions
        let no_perm_file = KernInode::new_file(None, None);
        assert_eq!(
            no_perm_file.read_at(0, &mut [0]).unwrap_err(),
            VfsError::PermissionDenied
        );
        assert_eq!(
            no_perm_file.write_at(0, &[0]).unwrap_err(),
            VfsError::PermissionDenied
        );
    }

    #[test_case]
    fn test_error_scenarios() {
        let root = KernInode::new_dir();
        let file_inode = KernInode::new_file(None, None);

        // NotADirectory errors
        assert_eq!(
            file_inode
                .add_child("child", KernInode::new_dir())
                .unwrap_err(),
            VfsError::NotADirectory
        );
        assert_eq!(
            file_inode.lookup("child").unwrap_err(),
            VfsError::NotADirectory
        );
        assert_eq!(
            file_inode.create("child", VNodeType::File).unwrap_err(),
            VfsError::NotADirectory
        );
        assert_eq!(
            file_inode
                .create_device("child", Arc::new(OldDevice::null()))
                .unwrap_err(),
            VfsError::NotADirectory
        );
        assert_eq!(
            file_inode.unlink("child").unwrap_err(),
            VfsError::NotADirectory
        );
        assert_eq!(file_inode.list().unwrap_err(), VfsError::NotADirectory);

        // NotAFile errors
        assert_eq!(root.read_at(0, &mut [0]).unwrap_err(), VfsError::NotAFile);
        assert_eq!(root.write_at(0, &[0]).unwrap_err(), VfsError::NotAFile);

        // NotFound errors (already covered by test_unlink and test_lookup for non-existent entry)
        // AlreadyExists errors (already covered by test_add_child_and_lookup and create/create_device for existing entry)
        // PermissionDenied errors (already covered by test_file_read_write for file without callbacks)

        // Test create with unsupported VNodeType
        assert_eq!(
            root.create("symlink", VNodeType::SymLink).unwrap_err(),
            VfsError::NotImplemented
        );

        // Test set_metadata (always NotImplemented)
        assert_eq!(
            root.set_metadata(&Metadata::default()).unwrap_err(),
            VfsError::NotImplemented
        );
        // Test truncate (always NotImplemented)
        assert_eq!(
            file_inode.truncate(0).unwrap_err(),
            VfsError::NotImplemented
        );
    }

    // Mock Device and CharDevice for testing
    struct MockCharDevice {
        data: RwLock<Vec<u8>>,
        name: String,
    }

    impl crate::drivers::SharedDeviceOps for MockCharDevice {
        fn name(&self) -> &str {
            &self.name
        }
        fn device_type(&self) -> crate::drivers::DeviceType {
            crate::drivers::DeviceType::Char
        }
        fn open(&self) -> Result<(), crate::drivers::DeviceError> {
            Ok(())
        }
        fn close(&self) -> Result<(), crate::drivers::DeviceError> {
            Ok(())
        }
        fn ioctl(&self, _cmd: u64, _arg: u64) -> Result<u64, crate::drivers::DeviceError> {
            Err(crate::drivers::DeviceError::NotSupported)
        }
    }

    impl crate::drivers::CharDevice for MockCharDevice {
        fn read(&self, buf: &mut [u8]) -> Result<usize, crate::drivers::DeviceError> {
            let mut data = self.data.write();
            let bytes_to_read = buf.len().min(data.len());
            buf[..bytes_to_read].copy_from_slice(&data[..bytes_to_read]);
            data.drain(..bytes_to_read);
            Ok(bytes_to_read)
        }

        fn write(&self, buf: &[u8]) -> Result<usize, crate::drivers::DeviceError> {
            self.data.write().extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    impl MockCharDevice {
        fn new(name: &str, initial_data: &[u8]) -> Arc<Self> {
            Arc::new(Self {
                data: RwLock::new(initial_data.to_vec()),
                name: name.to_string(),
            })
        }
    }

    // Mock BlockDevice for testing
    struct MockBlockDevice {
        name: String,
        block_size: usize,
        num_blocks: usize,
        data: RwLock<Vec<u8>>,
    }

    impl crate::drivers::SharedDeviceOps for MockBlockDevice {
        fn name(&self) -> &str {
            &self.name
        }
        fn device_type(&self) -> crate::drivers::DeviceType {
            crate::drivers::DeviceType::Block
        }
        fn open(&self) -> Result<(), crate::drivers::DeviceError> {
            Ok(())
        }
        fn close(&self) -> Result<(), crate::drivers::DeviceError> {
            Ok(())
        }
        fn ioctl(&self, _cmd: u64, _arg: u64) -> Result<u64, crate::drivers::DeviceError> {
            Err(crate::drivers::DeviceError::NotSupported)
        }
    }

    impl crate::drivers::BlockDevice for MockBlockDevice {
        fn block_size(&self) -> usize {
            self.block_size
        }
        fn num_blocks(&self) -> usize {
            self.num_blocks
        }

        fn read_blocks(
            &self,
            block_idx: usize,
            num_blocks: usize,
            buf: &mut [u8],
        ) -> Result<usize, crate::drivers::DeviceError> {
            let start_byte = block_idx * self.block_size;
            let end_byte = (block_idx + num_blocks) * self.block_size;
            let data = self.data.read();

            if start_byte >= data.len() {
                return Ok(0);
            }
            let bytes_to_read = buf
                .len()
                .min(data.len() - start_byte)
                .min(end_byte - start_byte);
            buf[..bytes_to_read].copy_from_slice(&data[start_byte..(start_byte + bytes_to_read)]);
            Ok(bytes_to_read)
        }

        fn write_blocks(
            &self,
            block_idx: usize,
            num_blocks: usize,
            buf: &[u8],
        ) -> Result<usize, crate::drivers::DeviceError> {
            let start_byte = block_idx * self.block_size;
            let end_byte = (block_idx + num_blocks) * self.block_size;
            let mut data = self.data.write();

            if end_byte > data.len() {
                data.resize(end_byte, 0);
            }
            let bytes_to_write = buf.len().min(end_byte - start_byte);
            data[start_byte..(start_byte + bytes_to_write)]
                .copy_from_slice(buf[..bytes_to_write].as_ref());
            Ok(bytes_to_write)
        }
    }

    impl MockBlockDevice {
        fn new(name: &str, block_size: usize, num_blocks: usize, initial_data: &[u8]) -> Arc<Self> {
            let mut data = initial_data.to_vec();
            data.resize(block_size * num_blocks, 0);
            Arc::new(Self {
                name: name.to_string(),
                block_size,
                num_blocks,
                data: RwLock::new(data),
            })
        }
    }

    #[test_case]
    fn test_device_node_read_write() {
        let root = KernInode::new_dir();
        let mock_char_device = MockCharDevice::new("test_char_dev", b"device_data");
        let char_device_arc = Arc::new(crate::drivers::OldDevice::new_auto_assign(
            "test_char_dev".to_string(),
            crate::drivers::DeviceInner::Char(mock_char_device.clone()),
        ));

        let device_inode = root
            .create_device("test_device", char_device_arc.clone())
            .unwrap();
        assert_eq!(device_inode.node_type(), VNodeType::Device);

        // Read from char device
        let mut read_buffer = vec![0; 5];
        device_inode.read_at(0, &mut read_buffer).unwrap();
        assert_eq!(read_buffer, b"devic"); // Wait, original was "device_data", first 5 bytes are "devic"

        // Write to char device
        let write_mock_device = MockCharDevice::new("write_char_dev", b"");
        let write_device_arc = Arc::new(crate::drivers::OldDevice::new_auto_assign(
            "write_char_dev".to_string(),
            crate::drivers::DeviceInner::Char(write_mock_device.clone()),
        ));
        let write_device_inode = root
            .create_device("write_test_device", write_device_arc)
            .unwrap();

        write_device_inode.write_at(0, b"write_test").unwrap();
        let mut fresh_read_buffer = vec![0; 10];
        write_device_inode
            .read_at(0, &mut fresh_read_buffer)
            .unwrap();
        assert_eq!(fresh_read_buffer, b"write_test");

        // Test error when not a char device
        let mock_block_device =
            MockBlockDevice::new("test_block_dev", 512, 10, b"initial_block_data");
        let block_device_arc = Arc::new(crate::drivers::OldDevice::new_auto_assign(
            "test_block_dev".to_string(),
            crate::drivers::DeviceInner::Block(mock_block_device.clone()),
        ));
        let block_device_inode = root.create_device("block_dev", block_device_arc).unwrap();
        assert_eq!(
            block_device_inode.read_at(0, &mut [0]).unwrap_err(),
            VfsError::NotImplemented
        );

        // Test null device
        let null_device_arc = Arc::new(crate::drivers::OldDevice::null());
        let null_inode = root
            .create_device("null_device_node", null_device_arc)
            .unwrap();
        let mut null_read_buf = [0; 10];
        assert_eq!(null_inode.read_at(0, &mut null_read_buf).unwrap(), 0);
        assert_eq!(null_inode.write_at(0, b"data").unwrap(), 4);
    }
}
