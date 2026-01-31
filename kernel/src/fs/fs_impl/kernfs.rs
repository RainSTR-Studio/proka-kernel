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
