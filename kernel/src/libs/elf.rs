extern crate alloc;
use core::fmt::Debug;
use elf_loader::{input::ElfBinary, Loader};

use crate::fs::vfs::{VfsError, VFS};
use crate::println;

/// ELF 加载区域的起始地址

pub enum ElfLoadError {
    Vfs(VfsError),
    Loader(elf_loader::Error),
}

impl From<VfsError> for ElfLoadError {
    fn from(err: VfsError) -> Self {
        ElfLoadError::Vfs(err)
    }
}

impl From<elf_loader::Error> for ElfLoadError {
    fn from(err: elf_loader::Error) -> Self {
        ElfLoadError::Loader(err)
    }
}

impl Debug for ElfLoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ElfLoadError::Vfs(err) => write!(f, "VfsError: {:?}", err),
            ElfLoadError::Loader(err) => write!(f, "LoaderError: {:?}", err),
        }
    }
}

/// 加载 ELF 动态库
///
/// # Arguments
/// * `path` - ELF 文件路径
///
/// # Returns
/// * 加载并重定位后的 ELF 库
pub fn load_elf(path: &str) -> Result<elf_loader::image::LoadedDylib<()>, ElfLoadError> {
    let f = VFS.open(path)?;
    let file_data = f.read_all()?;
    let data = file_data.as_slice();
    println!(
        "[elf] Loading ELF from '{}', size: {} bytes",
        path,
        data.len()
    );

    let mut loader = Loader::new();
    let e = loader
        .load_dylib(ElfBinary::new(path, data))?
        .relocator()
        .relocate()?;

    println!("[elf] ELF loaded and relocated successfully");
    Ok(e)
}

/// 测试函数：加载并执行一个简单的 ELF 动态库
pub fn test_load_elf() -> Result<i32, ElfLoadError> {
    let e = load_elf("mylib.so")?;
    println!("[elf] Resolving symbols...");
    let func = unsafe { e.get::<fn(i32, i32) -> i32>("add").unwrap() };
    let result = func(1, 2);
    println!("[elf] add(1, 2) = {}", result);
    assert_eq!(result, 3);
    Ok(result)
}
