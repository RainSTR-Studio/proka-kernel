use crate::drivers::{Device, DeviceError, DEVICE_MANAGER};
extern crate alloc;
use super::fs_impl;
use alloc::format;
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::any::Any;
use lazy_static::lazy_static;
use spin::{Mutex, RwLock};

lazy_static! {
    pub static ref VFS: Vfs = {
        let fs = Vfs::new();
        fs
    };
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VfsError {
    /// 文件或目录不存在
    NotFound,
    /// 文件或目录已存在
    AlreadyExists,
    /// 路径不是目录
    NotADirectory,
    /// 路径不是文件
    NotAFile,
    /// 权限不足
    PermissionDenied,
    /// 设备错误
    DeviceError(DeviceError),
    /// 无效的参数
    InvalidArgument,
    /// IO 错误
    IoError,
    /// 符号链接深度过深
    MaxSymlinkDepth,
    /// 文件系统类型不支持
    FsTypeNotSupported,
    /// 路径为空
    EmptyPath,
    /// 功能未实现
    NotImplemented,
    /// 目录非空
    DirectoryNotEmpty,
}

impl From<DeviceError> for VfsError {
    fn from(e: DeviceError) -> Self {
        VfsError::DeviceError(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VNodeType {
    /// 文件
    File,
    /// 目录
    Dir,
    /// 符号链接
    SymLink,
    /// 设备
    Device,
}

/// 文件或目录的元数据
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Metadata {
    /// 文件大小
    pub size: u64,
    /// UNIX权限位，如0o755
    pub permissions: u32,
    /// 用户ID
    pub uid: u32,
    /// 组ID
    pub gid: u32,
    /// 创建时间 (秒)
    pub ctime: u64,
    /// 最后修改时间 (秒)
    pub mtime: u64,
    /// 占用的块数
    pub blocks: u64,
    /// 硬链接数量
    pub nlinks: u64,
}

pub trait FileSystem: Send + Sync {
    fn mount(
        &self,
        device: Option<Arc<Device>>,
        args: Option<&[&str]>,
    ) -> Result<Arc<dyn Inode>, VfsError>;
    fn fs_type(&self) -> &'static str;
}

pub trait Inode: Send + Sync {
    fn metadata(&self) -> Result<Metadata, VfsError>;
    fn set_metadata(&self, metadata: &Metadata) -> Result<(), VfsError>;
    fn node_type(&self) -> VNodeType;
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError>;
    fn write_at(&self, offset: u64, buf: &[u8]) -> Result<usize, VfsError>;
    fn truncate(&self, size: u64) -> Result<(), VfsError>;
    fn sync(&self) -> Result<(), VfsError>;
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, VfsError>;
    fn create(&self, name: &str, typ: VNodeType) -> Result<Arc<dyn Inode>, VfsError>;

    fn create_symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, VfsError> {
        let _ = (name, target);
        Err(VfsError::NotImplemented)
    }

    fn create_device(&self, name: &str, device: Arc<Device>) -> Result<Arc<dyn Inode>, VfsError> {
        let _ = (name, device);
        Err(VfsError::NotImplemented)
    }

    fn unlink(&self, name: &str) -> Result<(), VfsError>;

    fn rename(&self, old_name: &str, new_name: &str) -> Result<(), VfsError> {
        let _ = (old_name, new_name);
        Err(VfsError::NotImplemented)
    }

    fn list(&self) -> Result<Vec<String>, VfsError> {
        Ok(Vec::new())
    }

    fn read_symlink(&self) -> Result<String, VfsError> {
        Err(VfsError::NotAFile)
    }

    fn as_any(&self) -> &dyn Any;
}

pub struct File {
    inode: Arc<dyn Inode>,
    offset: Mutex<u64>,
}

impl File {
    pub fn new(inode: Arc<dyn Inode>) -> Self {
        Self {
            inode,
            offset: Mutex::new(0),
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, VfsError> {
        let mut offset = self.offset.lock();
        let len = self.inode.read_at(*offset, buf)?;
        *offset += len as u64;
        Ok(len)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, VfsError> {
        let mut offset = self.offset.lock();
        let len = self.inode.write_at(*offset, buf)?;
        *offset += len as u64;
        Ok(len)
    }

    pub fn seek(&self, pos: u64) -> Result<u64, VfsError> {
        let mut offset = self.offset.lock();
        *offset = pos;
        Ok(*offset)
    }

    pub fn metadata(&self) -> Result<Metadata, VfsError> {
        self.inode.metadata()
    }

    pub fn truncate(&self, size: u64) -> Result<(), VfsError> {
        self.inode.truncate(size)
    }

    pub fn read_all(&self) -> Result<Vec<u8>, VfsError> {
        let metadata = self.metadata()?;
        let mut buf = alloc::vec![0; metadata.size as usize];
        self.inode.read_at(0, &mut buf)?;
        Ok(buf)
    }
    pub fn write_all(&self, data: &[u8]) -> Result<(), VfsError> {
        self.inode.write_at(0, data)?;
        Ok(())
    }
}

struct MountPoint {
    path: String,
    mount_point_len: usize,
    root: Arc<dyn Inode>,
}

pub struct Vfs {
    root: RwLock<Option<Arc<dyn Inode>>>,
    mounts: Mutex<Vec<MountPoint>>,
    fs_registry: RwLock<BTreeMap<&'static str, Arc<dyn FileSystem>>>,
}

impl Vfs {
    pub fn new() -> Self {
        let mut registry: BTreeMap<&'static str, Arc<dyn FileSystem>> = BTreeMap::new();
        let kernfs = Arc::new(fs_impl::kernfs::KernFs::new());
        registry.insert("kernfs", kernfs.clone());
        registry.insert("memfs", Arc::new(fs_impl::memfs::MemFs));

        let mounts = alloc::vec![MountPoint {
            path: "kernel".to_string(),
            mount_point_len: 6,
            root: kernfs.root(),
        }];

        Self {
            root: RwLock::new(None),
            mounts: Mutex::new(mounts),
            fs_registry: RwLock::new(registry),
        }
    }

    pub fn init_root(&self, root: Arc<dyn Inode>) {
        *self.root.write() = Some(root);
    }

    pub fn register_fs(&self, fs: Arc<dyn FileSystem>) {
        self.fs_registry.write().insert(fs.fs_type(), fs);
    }

    pub fn mount(
        &self,
        device_str: Option<&str>,
        mount_point: &str,
        fs_type: &str,
        args: Option<&[&str]>,
    ) -> Result<(), VfsError> {
        let fs = self
            .fs_registry
            .read()
            .get(fs_type)
            .cloned()
            .ok_or(VfsError::FsTypeNotSupported)?;

        let device_manager = DEVICE_MANAGER.read();
        let device = if let Some(dev) = device_str {
            device_manager.get_device(dev)
        } else {
            None
        };

        let root_inode = fs.mount(device, args)?;

        let parent_path = if mount_point == "/" {
            None
        } else {
            mount_point.rsplit_once('/').map(|(p, _)| p)
        };
        if let Some(parent_path_str) = parent_path {
            let parent_inode = self.lookup(parent_path_str)?;
            if parent_inode.node_type() != VNodeType::Dir {
                return Err(VfsError::NotADirectory);
            }
        }

        let normalized_mount_point = mount_point.trim_matches('/').to_string();
        if normalized_mount_point.is_empty() {
            return Err(VfsError::InvalidArgument);
        }

        self.mounts.lock().push(MountPoint {
            path: normalized_mount_point.clone(),
            mount_point_len: normalized_mount_point.len(),
            root: root_inode,
        });

        Ok(())
    }

    pub fn lookup(&self, path: &str) -> Result<Arc<dyn Inode>, VfsError> {
        self.path_to_inode(path, 0)
    }

    pub fn open(&self, path: &str) -> Result<Arc<File>, VfsError> {
        let inode = self.lookup(path)?;
        if inode.node_type() != VNodeType::File && inode.node_type() != VNodeType::Device {
            return Err(VfsError::NotAFile);
        }
        Ok(Arc::new(File::new(inode)))
    }

    pub fn create_file(&self, path: &str) -> Result<Arc<dyn Inode>, VfsError> {
        let (parent_path, name) = self.split_path(path)?;
        let parent_inode = self.lookup(parent_path)?;
        if parent_inode.node_type() != VNodeType::Dir {
            return Err(VfsError::NotADirectory);
        }
        parent_inode.create(name, VNodeType::File)
    }

    pub fn create_dir(&self, path: &str) -> Result<Arc<dyn Inode>, VfsError> {
        let (parent_path, name) = self.split_path(path)?;
        let parent_inode = self.lookup(parent_path)?;
        if parent_inode.node_type() != VNodeType::Dir {
            return Err(VfsError::NotADirectory);
        }
        parent_inode.create(name, VNodeType::Dir)
    }

    pub fn create_symlink(
        &self,
        target_path: &str,
        link_path: &str,
    ) -> Result<Arc<dyn Inode>, VfsError> {
        let (parent_path, name) = self.split_path(link_path)?;
        let parent_inode = self.lookup(parent_path)?;
        if parent_inode.node_type() != VNodeType::Dir {
            return Err(VfsError::NotADirectory);
        }
        parent_inode.create_symlink(name, target_path)
    }

    pub fn create_device_node(
        &self,
        path: &str,
        major: u16,
        minor: u16,
        device_type: crate::drivers::DeviceType,
    ) -> Result<Arc<dyn Inode>, VfsError> {
        let (parent_path, name) = self.split_path(path)?;
        let parent_inode = self.lookup(parent_path)?;
        if parent_inode.node_type() != VNodeType::Dir {
            return Err(VfsError::NotADirectory);
        }

        let device_manager = DEVICE_MANAGER.read();
        let device = device_manager
            .get_device_by_major_minor(major, minor)
            .ok_or(VfsError::DeviceError(
                crate::drivers::DeviceError::NoSuchDevice,
            ))?;

        if device.device_type() != device_type {
            return Err(VfsError::DeviceError(
                crate::drivers::DeviceError::InvalidParam,
            ));
        }

        parent_inode.create_device(name, device)
    }

    pub fn remove(&self, path: &str) -> Result<(), VfsError> {
        let (parent_path, name) = self.split_path(path)?;
        let parent_inode = self.lookup(parent_path)?;
        if parent_inode.node_type() != VNodeType::Dir {
            return Err(VfsError::NotADirectory);
        }
        parent_inode.unlink(name)
    }

    pub fn read_dir(&self, path: &str) -> Result<Vec<String>, VfsError> {
        let inode = self.lookup(path)?;
        if inode.node_type() != VNodeType::Dir {
            return Err(VfsError::NotADirectory);
        }
        inode.list()
    }

    pub fn rename(&self, old_path: &str, new_path: &str) -> Result<(), VfsError> {
        let (old_parent_path, old_name) = self.split_path(old_path)?;
        let old_parent = self.lookup(old_parent_path)?;

        let (new_parent_path, new_name) = self.split_path(new_path)?;
        let _new_parent = self.lookup(new_parent_path)?;

        if old_parent_path == new_parent_path {
            old_parent.rename(old_name, new_name)
        } else {
            Err(VfsError::NotImplemented)
        }
    }

    const MAX_SYMLINK_DEPTH: u32 = 8;

    fn path_to_inode(&self, path: &str, depth: u32) -> Result<Arc<dyn Inode>, VfsError> {
        if depth > Self::MAX_SYMLINK_DEPTH {
            return Err(VfsError::MaxSymlinkDepth);
        }

        let normalized_path = self.normalize_path(path);
        let path = normalized_path.trim_matches('/');

        if path.is_empty() {
            return self.root.read().as_ref().cloned().ok_or(VfsError::NotFound);
        }

        let mounts = self.mounts.lock();
        if let Some(mount) = mounts
            .iter()
            .filter(|m| path.starts_with(&m.path))
            .max_by_key(|m| m.mount_point_len)
        {
            let subpath = path[mount.mount_point_len..].trim_matches('/');
            if subpath.is_empty() {
                return self.resolve_symlink_if_needed(mount.root.clone(), depth);
            }
            let mut current = mount.root.clone();
            for component in subpath.split('/') {
                if component.is_empty() || component == "." {
                    continue;
                }
                current = self.resolve_symlink_if_needed(current.lookup(component)?, depth)?;
            }
            return Ok(current);
        }

        let mut current = self
            .root
            .read()
            .as_ref()
            .cloned()
            .ok_or(VfsError::NotFound)?;
        for component in path.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            current = self.resolve_symlink_if_needed(current.lookup(component)?, depth)?;
        }
        Ok(current)
    }

    fn resolve_symlink_if_needed(
        &self,
        node: Arc<dyn Inode>,
        depth: u32,
    ) -> Result<Arc<dyn Inode>, VfsError> {
        if node.node_type() == VNodeType::SymLink {
            let target_path = node.read_symlink()?;
            self.path_to_inode(&target_path, depth + 1)
        } else {
            Ok(node)
        }
    }

    fn split_path<'a>(&self, path: &'a str) -> Result<(&'a str, &'a str), VfsError> {
        let path = path.trim_matches('/');
        if path.is_empty() {
            return Err(VfsError::EmptyPath);
        }
        if let Some((parent, name)) = path.rsplit_once('/') {
            Ok(((if parent.is_empty() { "/" } else { parent }), name))
        } else {
            Ok(("/", path))
        }
    }

    pub fn walk(
        &self,
        start_path: &str,
    ) -> Result<Vec<(String, Vec<String>, Vec<String>)>, VfsError> {
        let mut results = Vec::new();
        let mut stack: Vec<String> = Vec::new();
        let normalized_start_path = self.normalize_path(start_path);
        stack.push(normalized_start_path);
        while let Some(current_dir_path) = stack.pop() {
            let current_inode = self.lookup(&current_dir_path)?;
            if current_inode.node_type() != VNodeType::Dir {
                continue;
            }
            let entries = current_inode.list()?;
            let mut dirnames: Vec<String> = Vec::new();
            let mut filenames: Vec<String> = Vec::new();
            let mut subdirs_to_visit: Vec<String> = Vec::new();

            for entry_name in entries {
                let entry_path = if current_dir_path == "/" {
                    format!("/{}", entry_name)
                } else {
                    format!("{}/{}", current_dir_path, entry_name)
                };

                let entry_inode = match self.lookup(&entry_path) {
                    Ok(node) => node,
                    Err(_) => continue,
                };

                match entry_inode.node_type() {
                    VNodeType::Dir => {
                        dirnames.push(entry_name.clone());
                        subdirs_to_visit.push(entry_path);
                    }
                    _ => {
                        filenames.push(entry_name.clone());
                    }
                }
            }
            results.push((current_dir_path.clone(), dirnames, filenames));
            subdirs_to_visit.sort_unstable_by(|a, b| b.cmp(a));
            for subdir_path in subdirs_to_visit {
                stack.push(subdir_path);
            }
        }
        Ok(results)
    }

    fn normalize_path(&self, path: &str) -> String {
        let parts: Vec<&str> = path
            .split('/')
            .filter(|&s| !s.is_empty() && s != ".")
            .collect();
        let mut cleaned_parts = Vec::new();
        for part in parts.iter() {
            if *part == ".." {
                if let Some(last) = cleaned_parts.last_mut() {
                    if last != &"" {
                        cleaned_parts.pop();
                    }
                }
            } else {
                cleaned_parts.push(*part);
            }
        }
        if cleaned_parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", cleaned_parts.join("/"))
        }
    }
}
