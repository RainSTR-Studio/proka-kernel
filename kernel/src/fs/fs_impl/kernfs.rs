extern crate alloc;
use crate::drivers::Device;
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
    /// 目录
    Dir(RwLock<BTreeMap<String, Arc<KernInode>>>),
    /// 读写函数
    File {
        read: Option<ReadCallback>,
        write: Option<WriteCallback>,
        size: u64,
    },
    /// 设备映射
    Device { device: Arc<Device> },
}

/// 内核文件系统节点
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
    /// 创建目录节点
    pub fn new_dir() -> Arc<Self> {
        Arc::new(Self {
            node_type: VNodeType::Dir,
            content: KernNodeContent::Dir(RwLock::new(BTreeMap::new())),
        })
    }

    /// 创建文件节点
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
    /// 创建设备节点
    pub fn new_device(device: Arc<Device>) -> Arc<Self> {
        Arc::new(Self {
            node_type: VNodeType::Device,
            content: KernNodeContent::Device { device },
        })
    }

    /// 添加子节点（仅目录节点可用）
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

    fn create_device(&self, name: &str, device: Arc<Device>) -> Result<Arc<dyn Inode>, VfsError> {
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

impl KernFs {
    pub fn new() -> Self {
        let root = KernInode::new_dir();

        Self { root }
    }

    pub fn root(&self) -> Arc<KernInode> {
        self.root.clone()
    }
}

impl FileSystem for KernFs {
    fn mount(
        &self,
        _device: Option<Arc<Device>>,
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
                .create_device("child", Arc::new(Device::null()))
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
}
