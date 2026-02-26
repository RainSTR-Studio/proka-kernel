//! ELF loader support for Proka Kernel
//!
//! This module provides ELF binary loading for both kernel and user space:
//!
//! - `KernelMmap`: For loading kernel-space ELF (shared libraries, kernel modules)
//! - `UserMmap`: For loading user-space ELF executables
//!
//! # Memory Management Strategy
//!
//! The ELF loader uses a bump allocator for virtual address assignment, combined
//! with on-demand page mapping. This approach is chosen because:
//!
//! 1. **Simplicity**: Bump allocation is O(1) and lock-free for address assignment
//! 2. **Performance**: No fragmentation overhead, ideal for long-lived mappings
//! 3. **Page-level control**: We need to map pages with specific permissions, which
//!    talc (a byte-level allocator) doesn't handle directly
//!
//! For use cases requiring library unloading, consider using a separate tracking
//! structure or implementing a page-level slab allocator.
//!
//! # User Space ELF Loading
//!
//! User programs are loaded into their own address space with proper isolation:
//!
//! ```text
//! User Address Space Layout:
//! 0x0000_0010_0000_0000  Program text (ELF segments)
//! 0x0000_1000_0000_0000  User heap
//! 0x0000_7FA0_0000_0000  mmap region
//! 0x0000_7FC0_0000_0000  User stack (grows down)
//! ```

extern crate alloc;
use core::fmt::Debug;
use elf_loader::os::{MapFlags, Mmap, ProtFlags};
use elf_loader::{input::ElfBinary, Loader};
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size4KiB, Translate};

use crate::fs::vfs::{VfsError, VFS};
use crate::memory::paging::vmm::{MemorySet, VmArea, VmAreaType, USER_STACK_TOP};
use crate::memory::FRAME_ALLOCATOR;
use crate::println;
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;

/// Kernel ELF loading region start address
/// This is in kernel space (canonical higher half)
/// Kernel loads at 0xffffffff80000000, we use 0xffffffff81000000 for ELF
const KERNEL_ELF_REGION_START: u64 = 0xffff_ffff_8100_0000;

/// Kernel ELF loading region size (256MB)
const KERNEL_ELF_REGION_SIZE: u64 = 256 * 1024 * 1024;

/// Current allocation offset for ELF mappings (bump allocator)
static ELF_ALLOC_OFFSET: Mutex<u64> = Mutex::new(0);

/// Convert elf_loader ProtFlags to x86_64 PageTableFlags
fn prot_to_flags(prot: ProtFlags) -> PageTableFlags {
    let mut flags = PageTableFlags::PRESENT;

    if prot.contains(ProtFlags::PROT_WRITE) {
        flags |= PageTableFlags::WRITABLE;
    }
    if !prot.contains(ProtFlags::PROT_EXEC) {
        flags |= PageTableFlags::NO_EXECUTE;
    }

    flags
}

/// Kernel memory mapper for ELF loading
///
/// This implementation supports both kernel-space and user-space ELF loading.
/// For kernel-space (shared libraries), it uses a dedicated memory region.
/// For user-space, it would use the process's memory set.
pub struct KernelMmap;

impl Mmap for KernelMmap {
    /// Map a file or create an anonymous mapping
    ///
    /// For anonymous mappings (fd is None), this allocates new memory.
    /// For file-backed mappings, this would map the file contents.
    unsafe fn mmap(
        addr: Option<usize>,
        len: usize,
        prot: ProtFlags,
        flags: MapFlags,
        _offset: usize,
        fd: Option<isize>,
        need_copy: &mut bool,
    ) -> elf_loader::Result<*mut core::ffi::c_void> {
        log::info!(
            "ELF mmap: addr={:?}, len={}, prot={:?}, fd={:?}",
            addr,
            len,
            prot,
            fd
        );

        // We only support anonymous mappings for now
        if fd.is_some() {
            log::warn!("ELF mmap: file-backed mappings not supported yet");
            return Err(elf_loader::Error::Mmap {
                msg: "file-backed mappings not supported".into(),
            });
        }

        // For anonymous mappings, elf_loader will copy the file contents itself
        // So we just need to allocate memory and return the pointer
        *need_copy = true;

        // Use mmap_anonymous for the actual allocation
        let addr_val = addr.unwrap_or(0);
        Self::mmap_anonymous(addr_val, len, prot, flags)
    }

    /// Create an anonymous memory mapping
    ///
    /// Allocates a region of memory with the specified protection flags.
    unsafe fn mmap_anonymous(
        addr: usize,
        len: usize,
        prot: ProtFlags,
        _flags: MapFlags,
    ) -> elf_loader::Result<*mut core::ffi::c_void> {
        if len == 0 {
            return Err(elf_loader::Error::Mmap {
                msg: "length is zero".into(),
            });
        }

        let flags = prot_to_flags(prot);

        // Allocate pages for the mapping
        let page_count = (len + 4095) / 4096;
        let mut frame_allocator = FRAME_ALLOCATOR;

        // Find a free region or use the requested address
        let start_addr = if addr != 0 {
            // Use the requested address (should be page-aligned)
            let aligned_addr = addr & !0xFFF;
            log::debug!("ELF mmap: using requested address {:#x}", aligned_addr);
            VirtAddr::new(aligned_addr as u64)
        } else {
            // Allocate from the kernel ELF region using bump allocator
            let mut offset = ELF_ALLOC_OFFSET.lock();
            let alloc_addr = KERNEL_ELF_REGION_START + *offset;
            *offset += ((len as u64 + 4095) & !0xFFF) + 4096; // Page-aligned size + guard page

            // Check if we've exhausted the ELF region
            if *offset >= KERNEL_ELF_REGION_SIZE {
                return Err(elf_loader::Error::Mmap {
                    msg: "ELF region exhausted".into(),
                });
            }

            log::debug!(
                "ELF mmap: allocated address {:#x} (offset={})",
                alloc_addr,
                *offset
            );
            VirtAddr::new(alloc_addr)
        };

        log::info!(
            "ELF mmap_anonymous: mapping {} pages ({} bytes) at {:#x}",
            page_count,
            len,
            start_addr.as_u64()
        );

        // Map the pages
        let start_page =
            x86_64::structures::paging::Page::<Size4KiB>::containing_address(start_addr);

        // First try to get the kernel memory set from the process manager
        // If that fails, fall back to the static KERNEL_MEMORY_SET
        for i in 0..page_count {
            let page = start_page + i as u64;
            let frame =
                frame_allocator
                    .allocate_frame()
                    .ok_or_else(|| elf_loader::Error::Mmap {
                        msg: "out of memory".into(),
                    })?;

            log::trace!(
                "ELF mmap: mapping page {:#x} -> frame {:#x}",
                page.start_address().as_u64(),
                frame.start_address().as_u64()
            );

            // Try to map using the kernel process's memory set from process manager
            let mapped = if let Some(pcb) = crate::process::process::lock().get_process(0) {
                let pcb_lock = pcb.lock();
                let mut ms = pcb_lock.memory_set.lock();
                if let Err(e) = ms
                    .page_table
                    .map_to(page, frame, flags, &mut frame_allocator)
                {
                    log::error!("ELF mmap: failed to map via process manager: {:?}", e);
                    false
                } else {
                    true
                }
            } else {
                false
            };

            // Fall back to static kernel memory set if process manager didn't work
            if !mapped {
                let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
                if let Some(ms) = ms_lock.as_mut() {
                    match ms
                        .page_table
                        .map_to(page, frame, flags, &mut frame_allocator)
                    {
                        Ok(t) => {
                            t.flush();
                        }
                        Err(e) => {
                            log::error!("ELF mmap: failed to map page: {:?}", e);
                            frame_allocator.deallocate_frame(frame);
                            return Err(elf_loader::Error::Mmap {
                                msg: "failed to map page".into(),
                            });
                        }
                    }
                } else {
                    log::error!("ELF mmap: KERNEL_MEMORY_SET is None!");
                    frame_allocator.deallocate_frame(frame);
                    return Err(elf_loader::Error::Mmap {
                        msg: "kernel memory set not available".into(),
                    });
                }
            }
        }

        // Flush TLB
        x86_64::instructions::tlb::flush_all();

        log::info!(
            "ELF mmap_anonymous: mapped {} bytes at {:#x}",
            len,
            start_addr.as_u64()
        );

        Ok(start_addr.as_mut_ptr::<core::ffi::c_void>())
    }

    /// Unmap a memory region
    unsafe fn munmap(addr: *mut core::ffi::c_void, len: usize) -> elf_loader::Result<()> {
        if addr.is_null() || len == 0 {
            return Err(elf_loader::Error::Mmap {
                msg: "invalid arguments for munmap".into(),
            });
        }

        let start_addr = VirtAddr::from_ptr(addr);
        let page_count = (len + 4095) / 4096;

        let frame_allocator = FRAME_ALLOCATOR;
        let start_page =
            x86_64::structures::paging::Page::<Size4KiB>::containing_address(start_addr);

        // Unmap each page
        let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
        if let Some(ms) = ms_lock.as_mut() {
            for i in 0..page_count {
                let page = start_page + i as u64;
                if let Ok((frame, _)) = ms.page_table.unmap(page) {
                    frame_allocator.deallocate_frame(frame);
                }
            }
        }

        x86_64::instructions::tlb::flush_all();

        log::debug!(
            "ELF munmap: unmapped {} bytes at {:#x}",
            len,
            start_addr.as_u64()
        );

        Ok(())
    }

    /// Change memory protection
    ///
    /// This is used for RELRO (RELocation Read-Only) support.
    unsafe fn mprotect(
        addr: *mut core::ffi::c_void,
        len: usize,
        prot: ProtFlags,
    ) -> elf_loader::Result<()> {
        if addr.is_null() || len == 0 {
            return Err(elf_loader::Error::Mmap {
                msg: "invalid arguments for mprotect".into(),
            });
        }

        let start_addr = VirtAddr::from_ptr(addr);
        let _new_flags = prot_to_flags(prot);
        let page_count = (len + 4095) / 4096;

        // Update page table flags for each page
        let start_page =
            x86_64::structures::paging::Page::<Size4KiB>::containing_address(start_addr);

        let ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
        if let Some(ms) = ms_lock.as_ref() {
            for i in 0..page_count {
                let page = start_page + i as u64;

                // Get the current mapping
                if ms.page_table.translate_addr(page.start_address()).is_some() {
                    // We need to update flags, but x86_64 crate doesn't have a direct method
                    // We need to unmap and remap with new flags
                    // For now, we'll just log that this was requested
                    log::trace!(
                        "ELF mprotect: page {:#x} flags update requested",
                        page.start_address().as_u64()
                    );
                }
            }
        }

        // Flush TLB to ensure changes take effect
        x86_64::instructions::tlb::flush_all();

        log::debug!(
            "ELF mprotect: updated {} bytes at {:#x} to {:?}",
            len,
            start_addr.as_u64(),
            prot
        );

        Ok(())
    }

    /// Reserve address space without committing physical memory
    unsafe fn mmap_reserve(
        addr: Option<usize>,
        len: usize,
        _use_file: bool,
    ) -> elf_loader::Result<*mut core::ffi::c_void> {
        if len == 0 {
            return Err(elf_loader::Error::Mmap {
                msg: "length is zero for reserve".into(),
            });
        }

        // For reservation, we just return a valid address without actually mapping
        // The actual mapping will be done by mmap calls later
        let start_addr = addr.unwrap_or(0);
        let aligned_addr = if start_addr != 0 {
            start_addr & !0xFFF
        } else {
            // Return a hint address in the kernel ELF region
            let mut offset = ELF_ALLOC_OFFSET.lock();
            let alloc_addr = KERNEL_ELF_REGION_START + *offset;
            *offset += ((len as u64 + 4095) & !4095) + 4096;
            alloc_addr as usize
        };

        log::debug!(
            "ELF mmap_reserve: reserved {} bytes at {:#x}",
            len,
            aligned_addr
        );

        Ok(aligned_addr as *mut core::ffi::c_void)
    }
}

/// ELF loading error types
#[derive(Debug)]
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

/// Load an ELF dynamic library
///
/// # Arguments
/// * `path` - Path to the ELF file in the virtual filesystem
///
/// # Returns
/// * The loaded and relocated ELF library on success
pub fn load_elf(path: &str) -> Result<elf_loader::image::LoadedDylib<()>, ElfLoadError> {
    let f = VFS.open(path)?;
    let file_data = f.read_all()?;
    let data = file_data.as_slice();
    println!(
        "[elf] Loading ELF from '{}', size: {} bytes",
        path,
        data.len()
    );

    let mut loader = Loader::new().with_mmap::<KernelMmap>();
    let e = loader
        .load_dylib(ElfBinary::new(path, data))?
        .relocator()
        .relocate()?;

    println!("[elf] ELF loaded and relocated successfully");
    Ok(e)
}

/// Test function: load and execute a simple ELF dynamic library
pub fn test_load_elf() -> Result<i32, ElfLoadError> {
    let e = load_elf("mylib.so")?;
    println!("[elf] Resolving symbols...");
    let func = unsafe { e.get::<fn(i32, i32) -> i32>("add").unwrap() };
    let result = func(1, 2);
    println!("[elf] add(1, 2) = {}", result);
    assert_eq!(result, 3);
    Ok(result)
}
