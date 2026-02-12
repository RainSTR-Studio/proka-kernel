extern crate alloc;
use crate::drivers::OldDevice;
use crate::fs::vfs::{FileSystem, Inode, Metadata, VNodeType, VfsError};
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::any::Any;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::RwLock;

static NEXT_INODE_ID: AtomicUsize = AtomicUsize::new(1);

fn create_metadata(node_type: VNodeType, size: u64) -> Metadata {
    let permissions = match node_type {
        VNodeType::Dir => 0o755,
        VNodeType::File => 0o644,
        VNodeType::SymLink => 0o777,
        VNodeType::Device => 0o600,
    };
    Metadata {
        size,
        permissions,
        uid: 0,
        gid: 0,
        ctime: 0,
        mtime: 0,
        blocks: size.div_ceil(512),
        nlinks: 1,
    }
}

#[derive(Debug)]
pub enum MemNodeContent {
    File {
        data: Arc<RwLock<Vec<u8>>>,
    },
    Dir {
        entries: RwLock<BTreeMap<String, Arc<MemVNode>>>,
    },
    SymLink {
        target: String,
    },
    Device {
        device: Arc<OldDevice>,
    },
}

#[derive(Debug)]
pub struct MemVNode {
    #[allow(dead_code)]
    id: usize,
    node_type: VNodeType,
    metadata: RwLock<Metadata>,
    content: MemNodeContent,
}

impl MemVNode {
    fn new(node_type: VNodeType, content: MemNodeContent) -> Arc<Self> {
        let id = NEXT_INODE_ID.fetch_add(1, Ordering::Relaxed);
        let size = match &content {
            MemNodeContent::File { data } => data.read().len() as u64,
            MemNodeContent::Dir { .. } => 0,
            MemNodeContent::SymLink { target } => target.len() as u64,
            MemNodeContent::Device { .. } => 0,
        };
        Arc::new(Self {
            id,
            node_type,
            metadata: RwLock::new(create_metadata(node_type, size)),
            content,
        })
    }

    fn update_mtime(&self) {}

    fn update_size(&self, new_size: u64) {
        let mut meta = self.metadata.write();
        meta.size = new_size;
        meta.blocks = new_size.div_ceil(512);
    }

    pub fn move_node(
        source_parent: &Self,
        target_parent: &Self,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), VfsError> {
        let source_entries = match &source_parent.content {
            MemNodeContent::Dir { entries } => entries,
            _ => return Err(VfsError::NotADirectory),
        };

        let target_entries = match &target_parent.content {
            MemNodeContent::Dir { entries } => entries,
            _ => return Err(VfsError::NotADirectory),
        };

        let node_to_move = {
            let source_read = source_entries.read();
            source_read.get(old_name).ok_or(VfsError::NotFound)?.clone()
        };

        {
            let target_read = target_entries.read();
            if target_read.contains_key(new_name) {
                return Err(VfsError::AlreadyExists);
            }
        }

        {
            let mut source_write = source_entries.write();
            source_write.remove(old_name);
        }

        {
            let mut target_write = target_entries.write();
            target_write.insert(new_name.to_string(), node_to_move);
        }

        source_parent.update_mtime();
        target_parent.update_mtime();

        Ok(())
    }
}

impl Inode for MemVNode {
    fn metadata(&self) -> Result<Metadata, VfsError> {
        Ok(self.metadata.read().clone())
    }

    fn set_metadata(&self, metadata: &Metadata) -> Result<(), VfsError> {
        *self.metadata.write() = metadata.clone();
        Ok(())
    }

    fn node_type(&self) -> VNodeType {
        self.node_type
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        match &self.content {
            MemNodeContent::File { data } => {
                let data = data.read();
                let data_len = data.len() as u64;
                if offset >= data_len {
                    return Ok(0);
                }
                let bytes_to_read = (data_len - offset).min(buf.len() as u64) as usize;
                let start = offset as usize;
                let end = start + bytes_to_read;
                buf[..bytes_to_read].copy_from_slice(&data[start..end]);
                Ok(bytes_to_read)
            }
            MemNodeContent::Device { device } => {
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
            MemNodeContent::File { data } => {
                let mut data = data.write();
                let start = offset as usize;
                let end = start + buf.len();

                if end > data.len() {
                    data.resize(end, 0);
                }

                data[start..end].copy_from_slice(buf);

                let new_len = data.len() as u64;
                drop(data);
                self.update_size(new_len);

                Ok(buf.len())
            }
            MemNodeContent::Device { device } => {
                if let Some(char_dev) = device.as_char_device() {
                    char_dev.write(buf).map_err(VfsError::DeviceError)
                } else {
                    Err(VfsError::NotImplemented)
                }
            }
            _ => Err(VfsError::NotAFile),
        }
    }

    fn truncate(&self, size: u64) -> Result<(), VfsError> {
        match &self.content {
            MemNodeContent::File { data } => {
                let mut data = data.write();
                data.resize(size as usize, 0);
                let new_len = data.len() as u64;
                drop(data);
                self.update_size(new_len);
                Ok(())
            }
            _ => Err(VfsError::NotImplemented),
        }
    }

    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => {
                let entries = entries.read();
                entries
                    .get(name)
                    .cloned()
                    .map(|node| node as Arc<dyn Inode>)
                    .ok_or(VfsError::NotFound)
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn create(&self, name: &str, typ: VNodeType) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => {
                let mut entries = entries.write();
                if entries.contains_key(name) {
                    return Err(VfsError::AlreadyExists);
                }

                let new_node = match typ {
                    VNodeType::File => MemVNode::new(
                        VNodeType::File,
                        MemNodeContent::File {
                            data: Arc::new(RwLock::new(Vec::new())),
                        },
                    ),
                    VNodeType::Dir => MemVNode::new(
                        VNodeType::Dir,
                        MemNodeContent::Dir {
                            entries: RwLock::new(BTreeMap::new()),
                        },
                    ),
                    _ => return Err(VfsError::NotImplemented),
                };

                entries.insert(name.to_string(), new_node.clone());
                self.update_mtime();
                Ok(new_node)
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn create_symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => {
                let mut entries = entries.write();
                if entries.contains_key(name) {
                    return Err(VfsError::AlreadyExists);
                }
                let new_node = MemVNode::new(
                    VNodeType::SymLink,
                    MemNodeContent::SymLink {
                        target: target.to_string(),
                    },
                );
                entries.insert(name.to_string(), new_node.clone());
                self.update_mtime();
                Ok(new_node)
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn create_device(&self, name: &str, device: Arc<OldDevice>) -> Result<Arc<dyn Inode>, VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => {
                let mut entries = entries.write();
                if entries.contains_key(name) {
                    return Err(VfsError::AlreadyExists);
                }

                let new_node = MemVNode::new(VNodeType::Device, MemNodeContent::Device { device });
                entries.insert(name.to_string(), new_node.clone());
                self.update_mtime();
                Ok(new_node)
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn unlink(&self, name: &str) -> Result<(), VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => {
                let mut entries = entries.write();
                if let Some(node) = entries.get(name) {
                    if node.node_type() == VNodeType::Dir {
                        if let MemNodeContent::Dir {
                            entries: sub_entries,
                        } = &node.content
                        {
                            if !sub_entries.read().is_empty() {
                                return Err(VfsError::DirectoryNotEmpty);
                            }
                        }
                    }
                    entries.remove(name);
                    self.update_mtime();
                    Ok(())
                } else {
                    Err(VfsError::NotFound)
                }
            }
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn rename(&self, old_name: &str, new_name: &str) -> Result<(), VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => {
                let mut entries = entries.write();
                if !entries.contains_key(old_name) {
                    return Err(VfsError::NotFound);
                }
                if entries.contains_key(new_name) {
                    return Err(VfsError::AlreadyExists);
                }
                let node = entries.remove(old_name).unwrap();
                entries.insert(new_name.to_string(), node);
                self.update_mtime();
                Ok(())
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
        if let Some(target_mem) = target_dir.as_any().downcast_ref::<MemVNode>() {
            Self::move_node(self, target_mem, old_name, new_name)
        } else {
            Err(VfsError::NotImplemented)
        }
    }

    fn list(&self) -> Result<Vec<String>, VfsError> {
        match &self.content {
            MemNodeContent::Dir { entries } => Ok(entries.read().keys().cloned().collect()),
            _ => Err(VfsError::NotADirectory),
        }
    }

    fn read_symlink(&self) -> Result<String, VfsError> {
        match &self.content {
            MemNodeContent::SymLink { target } => Ok(target.clone()),
            _ => Err(VfsError::NotAFile),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct MemFs;

impl FileSystem for MemFs {
    fn mount(
        &self,
        _device: Option<Arc<OldDevice>>,
        _args: Option<&[&str]>,
    ) -> Result<Arc<dyn Inode>, VfsError> {
        let root_dir = MemVNode::new(
            VNodeType::Dir,
            MemNodeContent::Dir {
                entries: RwLock::new(BTreeMap::new()),
            },
        );
        Ok(root_dir)
    }

    fn fs_type(&self) -> &'static str {
        "memfs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test_case]
    fn test_mount_creates_root() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();
        assert_eq!(root.node_type(), VNodeType::Dir);
    }

    #[test_case]
    fn test_create_file() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let file = root.create("test.txt", VNodeType::File).unwrap();
        assert_eq!(file.node_type(), VNodeType::File);

        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.size, 0);
    }

    #[test_case]
    fn test_create_dir() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let dir = root.create("testdir", VNodeType::Dir).unwrap();
        assert_eq!(dir.node_type(), VNodeType::Dir);

        let metadata = dir.metadata().unwrap();
        assert_eq!(metadata.size, 0);
    }

    #[test_case]
    fn test_create_duplicate_fails() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        root.create("test.txt", VNodeType::File).unwrap();
        let result = root.create("test.txt", VNodeType::File);
        assert!(matches!(result, Err(VfsError::AlreadyExists)));
    }

    #[test_case]
    fn test_lookup() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        root.create("test.txt", VNodeType::File).unwrap();
        let file = root.lookup("test.txt").unwrap();
        assert_eq!(file.node_type(), VNodeType::File);
    }

    #[test_case]
    fn test_lookup_not_found() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let result = root.lookup("nonexistent.txt");
        assert!(matches!(result, Err(VfsError::NotFound)));
    }

    #[test_case]
    fn test_write_and_read() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let file = root.create("test.txt", VNodeType::File).unwrap();
        let data = b"Hello, World!";
        let written = file.write_at(0, data).unwrap();
        assert_eq!(written, data.len());

        let mut buffer = vec![0u8; data.len()];
        let read = file.read_at(0, &mut buffer).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buffer, data);

        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.size, data.len() as u64);
    }

    #[test_case]
    fn test_write_at_offset() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let file = root.create("test.txt", VNodeType::File).unwrap();
        file.write_at(0, b"Hello, ").unwrap();
        file.write_at(7, b"World!").unwrap();

        let mut buffer = vec![0u8; 13];
        file.read_at(0, &mut buffer).unwrap();
        assert_eq!(&buffer, b"Hello, World!");
    }

    #[test_case]
    fn test_read_beyond_file() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let file = root.create("test.txt", VNodeType::File).unwrap();
        file.write_at(0, b"Hello").unwrap();

        let mut buffer = vec![0u8; 10];
        let read = file.read_at(0, &mut buffer).unwrap();
        assert_eq!(read, 5);
    }

    #[test_case]
    fn test_truncate() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let file = root.create("test.txt", VNodeType::File).unwrap();
        file.write_at(0, b"Hello, World!").unwrap();

        file.truncate(5).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.size, 5);

        let mut buffer = vec![0u8; 10];
        let read = file.read_at(0, &mut buffer).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buffer[..5], b"Hello");
    }

    #[test_case]
    fn test_truncate_extend() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let file = root.create("test.txt", VNodeType::File).unwrap();
        file.write_at(0, b"Hello").unwrap();

        file.truncate(10).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.size, 10);

        let mut buffer = vec![0u8; 10];
        file.read_at(0, &mut buffer).unwrap();
        assert_eq!(&buffer[..5], b"Hello");
        assert_eq!(&buffer[5..], b"\x00\x00\x00\x00\x00");
    }

    #[test_case]
    fn test_list() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        root.create("file1.txt", VNodeType::File).unwrap();
        root.create("file2.txt", VNodeType::File).unwrap();
        root.create("dir1", VNodeType::Dir).unwrap();

        let entries = root.list().unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&String::from("file1.txt")));
        assert!(entries.contains(&String::from("file2.txt")));
        assert!(entries.contains(&String::from("dir1")));
    }

    #[test_case]
    fn test_unlink() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        root.create("test.txt", VNodeType::File).unwrap();
        root.unlink("test.txt").unwrap();

        let result = root.lookup("test.txt");
        assert!(matches!(result, Err(VfsError::NotFound)));
    }

    #[test_case]
    fn test_unlink_not_found() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let result = root.unlink("nonexistent.txt");
        assert!(matches!(result, Err(VfsError::NotFound)));
    }

    #[test_case]
    fn test_rename() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        root.create("old.txt", VNodeType::File).unwrap();
        root.rename("old.txt", "new.txt").unwrap();

        let result = root.lookup("old.txt");
        assert!(matches!(result, Err(VfsError::NotFound)));

        let file = root.lookup("new.txt").unwrap();
        assert_eq!(file.node_type(), VNodeType::File);
    }

    #[test_case]
    fn test_rename_not_found() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let result = root.rename("old.txt", "new.txt");
        assert!(matches!(result, Err(VfsError::NotFound)));
    }

    #[test_case]
    fn test_rename_already_exists() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        root.create("file1.txt", VNodeType::File).unwrap();
        root.create("file2.txt", VNodeType::File).unwrap();

        let result = root.rename("file1.txt", "file2.txt");
        assert!(matches!(result, Err(VfsError::AlreadyExists)));
    }

    #[test_case]
    fn test_create_symlink() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let link = root.create_symlink("link.txt", "target.txt").unwrap();
        assert_eq!(link.node_type(), VNodeType::SymLink);

        let target = link.read_symlink().unwrap();
        assert_eq!(target, "target.txt");
    }

    #[test_case]
    fn test_directory_operations() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let dir = root.create("testdir", VNodeType::Dir).unwrap();

        // File operations on directory should fail
        let result = dir.read_at(0, &mut [0u8; 10]);
        assert!(matches!(result, Err(VfsError::NotAFile)));

        let result = dir.write_at(0, b"test");
        assert!(matches!(result, Err(VfsError::NotAFile)));
    }

    #[test_case]
    fn test_nested_directories() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let dir1 = root.create("dir1", VNodeType::Dir).unwrap();
        let dir2 = dir1.create("dir2", VNodeType::Dir).unwrap();
        let file = dir2.create("file.txt", VNodeType::File).unwrap();

        file.write_at(0, b"nested").unwrap();

        let lookup = dir2.lookup("file.txt").unwrap();
        let mut buffer = vec![0u8; 6];
        lookup.read_at(0, &mut buffer).unwrap();
        assert_eq!(&buffer, b"nested");
    }

    #[test_case]
    fn test_unlink_non_empty_dir() {
        let fs = MemFs;
        let root = fs.mount(None, None).unwrap();

        let dir = root.create("testdir", VNodeType::Dir).unwrap();
        dir.create("file.txt", VNodeType::File).unwrap();

        let result = root.unlink("testdir");
        assert!(matches!(result, Err(VfsError::DirectoryNotEmpty)));
    }
}
