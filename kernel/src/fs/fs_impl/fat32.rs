#![allow(dead_code)]

extern crate alloc;

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::any::Any;
use spin::RwLock;

use crate::drivers::Device;
use crate::fs::vfs::{FileSystem, Inode, Metadata, VNodeType, VfsError};

/// ================= BPB =================

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct BiosParameterBlock {
    jmp: [u8; 3],
    oem: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fats: u8,
    root_entries: u16,
    total_sectors_16: u16,
    media: u8,
    fat_size_16: u16,
    sectors_per_track: u16,
    heads: u16,
    hidden_sectors: u32,
    total_sectors_32: u32,

    fat_size_32: u32,
    ext_flags: u16,
    fs_version: u16,
    root_cluster: u32,
    fs_info: u16,
    backup_boot: u16,
    _reserved: [u8; 12],
}

/// ================= FAT32 FS =================

pub struct Fat32Fs {
    device: Arc<Device>,
    bpb: BiosParameterBlock,
    fat_start_lba: u64,
    data_start_lba: u64,
}

impl Fat32Fs {
    fn read_sector(&self, lba: u64, buf: &mut [u8]) -> Result<(), VfsError> {
        self.device
            .read_at(lba * 512, buf)
            .map_err(VfsError::DeviceError)?;
        Ok(())
    }

    fn cluster_size(&self) -> u64 {
        self.bpb.bytes_per_sector as u64 * self.bpb.sectors_per_cluster as u64
    }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.data_start_lba + (cluster as u64 - 2) * self.bpb.sectors_per_cluster as u64
    }

    fn next_cluster(&self, cluster: u32) -> Result<u32, VfsError> {
        let fat_offset = cluster as u64 * 4;
        let sector = self.fat_start_lba + fat_offset / 512;
        let offset = (fat_offset % 512) as usize;

        let mut buf = [0u8; 512];
        self.read_sector(sector, &mut buf)?;

        Ok(u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]) & 0x0FFFFFFF)
    }
}

/// ================= 目录项 =================

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct DirEntry {
    name: [u8; 11],
    attr: u8,
    _nt: u8,
    _ctime_tenth: u8,
    _ctime: u16,
    _cdate: u16,
    _adate: u16,
    cluster_hi: u16,
    _mtime: u16,
    _mdate: u16,
    cluster_lo: u16,
    size: u32,
}

impl DirEntry {
    fn is_unused(&self) -> bool {
        self.name[0] == 0x00 || self.name[0] == 0xE5
    }

    fn is_dir(&self) -> bool {
        self.attr & 0x10 != 0
    }

    fn first_cluster(&self) -> u32 {
        ((self.cluster_hi as u32) << 16) | self.cluster_lo as u32
    }

    fn filename(&self) -> String {
        let name = &self.name[0..8];
        let ext = &self.name[8..11];
        let n = core::str::from_utf8(name).unwrap().trim();
        let e = core::str::from_utf8(ext).unwrap().trim();
        if e.is_empty() {
            n.to_string()
        } else {
            format!("{}.{}", n, e)
        }
    }
}

/// ================= Inode =================

pub struct Fat32Inode {
    fs: Arc<Fat32Fs>,
    cluster: u32,
    is_dir: bool,
    metadata: RwLock<Metadata>,
}

impl Fat32Inode {
    fn read_dir_entries(&self) -> Result<Vec<DirEntry>, VfsError> {
        let mut entries = Vec::new();
        let mut cluster = self.cluster;
        let cluster_size = self.fs.cluster_size() as usize;

        while cluster < 0x0FFFFFF8 {
            let lba = self.fs.cluster_to_lba(cluster);
            let mut buf = vec![0u8; cluster_size];

            self.fs
                .device
                .read_at(lba * 512, &mut buf)
                .map_err(VfsError::DeviceError)?;

            let mut offset = 0;
            while offset + 32 <= buf.len() {
                let entry: DirEntry =
                    unsafe { core::ptr::read(buf[offset..].as_ptr() as *const _) };
                if entry.is_unused() {
                    offset += 32;
                    continue;
                }
                entries.push(entry);
                offset += 32;
            }

            cluster = self.fs.next_cluster(cluster)?;
        }

        Ok(entries)
    }
}

impl Inode for Fat32Inode {
    fn metadata(&self) -> Result<Metadata, VfsError> {
        Ok(self.metadata.read().clone())
    }

    fn set_metadata(&self, _: &Metadata) -> Result<(), VfsError> {
        Err(VfsError::NotImplemented)
    }

    fn node_type(&self) -> VNodeType {
        if self.is_dir {
            VNodeType::Dir
        } else {
            VNodeType::File
        }
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        if self.is_dir {
            return Err(VfsError::NotAFile);
        }

        let mut cluster = self.cluster;
        let cluster_size = self.fs.cluster_size();
        let mut skip = offset;
        let mut read = 0;

        while skip >= cluster_size {
            cluster = self.fs.next_cluster(cluster)?;
            skip -= cluster_size;
        }

        while read < buf.len() {
            let lba = self.fs.cluster_to_lba(cluster);
            let inner = skip as usize;
            let max = core::cmp::min((cluster_size - skip) as usize, buf.len() - read);

            self.fs
                .device
                .read_at(lba * 512 + inner as u64, &mut buf[read..read + max])
                .map_err(VfsError::DeviceError)?;

            read += max;
            skip = 0;

            cluster = self.fs.next_cluster(cluster)?;
            if cluster >= 0x0FFFFFF8 {
                break;
            }
        }

        Ok(read)
    }

    fn write_at(&self, _: u64, _: &[u8]) -> Result<usize, VfsError> {
        Err(VfsError::PermissionDenied)
    }

    fn truncate(&self, _: u64) -> Result<(), VfsError> {
        Err(VfsError::NotImplemented)
    }

    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, VfsError> {
        if !self.is_dir {
            return Err(VfsError::NotADirectory);
        }

        for e in self.read_dir_entries()? {
            if e.filename() == name {
                let is_dir = e.is_dir();
                let meta = Metadata {
                    size: e.size as u64,
                    permissions: if is_dir { 0o755 } else { 0o644 },
                    uid: 0,
                    gid: 0,
                    ctime: 0,
                    mtime: 0,
                    blocks: 0,
                    nlinks: 1,
                };

                return Ok(Arc::new(Fat32Inode {
                    fs: self.fs.clone(),
                    cluster: e.first_cluster(),
                    is_dir,
                    metadata: RwLock::new(meta),
                }));
            }
        }

        Err(VfsError::NotFound)
    }

    fn create(&self, _: &str, _: VNodeType) -> Result<Arc<dyn Inode>, VfsError> {
        Err(VfsError::PermissionDenied)
    }

    fn unlink(&self, _: &str) -> Result<(), VfsError> {
        Err(VfsError::PermissionDenied)
    }

    fn list(&self) -> Result<Vec<String>, VfsError> {
        if !self.is_dir {
            return Err(VfsError::NotADirectory);
        }
        Ok(self
            .read_dir_entries()?
            .into_iter()
            .map(|e| e.filename())
            .collect())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// ================= FileSystem =================

pub struct Fat32;

impl FileSystem for Fat32 {
    fn mount(
        &self,
        device: Option<Arc<Device>>,
        _: Option<&[&str]>,
    ) -> Result<Arc<dyn Inode>, VfsError> {
        let device = device.ok_or(VfsError::InvalidArgument)?;

        let mut buf = [0u8; 512];
        device.read_at(0, &mut buf).map_err(VfsError::DeviceError)?;

        let bpb: BiosParameterBlock = unsafe { core::ptr::read(buf.as_ptr() as *const _) };

        let fat_start = bpb.reserved_sectors as u64;
        let data_start = fat_start + bpb.fats as u64 * bpb.fat_size_32 as u64;

        let fs = Arc::new(Fat32Fs {
            device,
            bpb,
            fat_start_lba: fat_start,
            data_start_lba: data_start,
        });

        Ok(Arc::new(Fat32Inode {
            fs,
            cluster: bpb.root_cluster,
            is_dir: true,
            metadata: RwLock::new(Metadata {
                size: 0,
                permissions: 0o755,
                uid: 0,
                gid: 0,
                ctime: 0,
                mtime: 0,
                blocks: 0,
                nlinks: 1,
            }),
        }))
    }

    fn fs_type(&self) -> &'static str {
        "fat32"
    }
}
