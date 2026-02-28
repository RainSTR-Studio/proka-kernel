//! Memory Service
//!
//! Handles memory management operations:
//! - mmap: Map memory into user address space
//! - munmap: Unmap memory from user address space
//! - brk: Adjust heap break

use super::error;
use super::{IpcRequest, IpcResponse, Service, ServiceId};

use crate::memory::paging::vmm::USER_SPACE_END;
use crate::process;
use crate::process::thread::Tid;
use alloc::sync::Arc;
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;

/// mmap protection flags (matches Linux values)
const PROT_NONE: u64 = 0x0;
const PROT_READ: u64 = 0x1;
const PROT_WRITE: u64 = 0x2;
const PROT_EXEC: u64 = 0x4;

/// mmap flags (matches Linux values)
const MAP_SHARED: u64 = 0x01;
const MAP_PRIVATE: u64 = 0x02;
const MAP_FIXED: u64 = 0x10;
const MAP_ANONYMOUS: u64 = 0x20;

/// Memory service implementation
pub struct MemoryService;

impl MemoryService {
    /// Create a new memory service
    pub fn new() -> Self {
        Self
    }

    /// Handle mmap request
    ///
    /// Payload format (48 bytes):
    /// - bytes 0-7: addr (u64)
    /// - bytes 8-15: size (u64)
    /// - bytes 16-23: prot (u64)
    /// - bytes 24-31: flags (u64)
    /// - bytes 32-39: fd (u64, unused)
    /// - bytes 40-47: offset (u64, unused)
    fn handle_mmap(&self, _sender: Tid, request: &IpcRequest) -> IpcResponse {
        if request.payload.len() < 32 {
            log::warn!("MemoryService::mmap: payload too short");
            return IpcResponse::error(error::EINVAL);
        }

        let addr = u64::from_le_bytes(request.payload[0..8].try_into().unwrap());
        let size = u64::from_le_bytes(request.payload[8..16].try_into().unwrap()) as usize;
        let prot = u64::from_le_bytes(request.payload[16..24].try_into().unwrap());
        let flags = u64::from_le_bytes(request.payload[24..32].try_into().unwrap());

        // Validate size
        if size == 0 {
            log::warn!("MemoryService::mmap: size is 0");
            return IpcResponse::ok(!0u64); // -1
        }

        // We only support anonymous mappings for now
        if flags & MAP_ANONYMOUS == 0 {
            log::warn!("MemoryService::mmap: only anonymous mappings supported");
            return IpcResponse::ok(!0u64);
        }

        // Validate address if MAP_FIXED is specified
        let preferred_addr = if addr != 0 {
            if addr >= USER_SPACE_END {
                log::warn!("MemoryService::mmap: address {:#x} out of user space", addr);
                return IpcResponse::ok(!0u64);
            }
            Some(VirtAddr::new(addr))
        } else {
            None
        };

        // Convert protection flags to PageTableFlags
        let mut pt_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if prot & PROT_WRITE != 0 {
            pt_flags |= PageTableFlags::WRITABLE;
        }
        if prot & PROT_EXEC == 0 {
            pt_flags |= PageTableFlags::NO_EXECUTE;
        }

        // Get current process's memory set
        let pcb = match process::current_process() {
            Some(pcb) => pcb,
            None => {
                log::warn!("MemoryService::mmap: no current process");
                return IpcResponse::ok(!0u64);
            }
        };

        let pcb_lock = pcb.lock();
        let memory_set = Arc::clone(&pcb_lock.memory_set);
        drop(pcb_lock);

        let mut ms = memory_set.lock();

        match ms.mmap_anon(preferred_addr, size, pt_flags) {
            Ok(mapped_addr) => {
                log::debug!(
                    "MemoryService::mmap: mapped {} bytes at {:#x}",
                    size,
                    mapped_addr.as_u64()
                );
                IpcResponse::ok(mapped_addr.as_u64())
            }
            Err(e) => {
                log::warn!("MemoryService::mmap failed: {:?}", e);
                IpcResponse::ok(!0u64)
            }
        }
    }

    /// Handle munmap request
    ///
    /// Payload format (16 bytes):
    /// - bytes 0-7: addr (u64)
    /// - bytes 8-15: size (u64)
    fn handle_munmap(&self, _sender: Tid, request: &IpcRequest) -> IpcResponse {
        if request.payload.len() < 16 {
            log::warn!("MemoryService::munmap: payload too short");
            return IpcResponse::error(error::EINVAL);
        }

        let addr = u64::from_le_bytes(request.payload[0..8].try_into().unwrap());
        let size = u64::from_le_bytes(request.payload[8..16].try_into().unwrap()) as usize;

        // Validate address is in user space
        if addr >= USER_SPACE_END {
            log::warn!(
                "MemoryService::munmap: address {:#x} out of user space",
                addr
            );
            return IpcResponse::ok(!0u64);
        }

        // Validate size
        if size == 0 {
            log::warn!("MemoryService::munmap: size is 0");
            return IpcResponse::ok(!0u64);
        }

        // Get current process's memory set
        let pcb = match process::current_process() {
            Some(pcb) => pcb,
            None => {
                log::warn!("MemoryService::munmap: no current process");
                return IpcResponse::ok(!0u64);
            }
        };

        let pcb_lock = pcb.lock();
        let memory_set = Arc::clone(&pcb_lock.memory_set);
        drop(pcb_lock);

        let mut ms = memory_set.lock();

        match ms.munmap(VirtAddr::new(addr), size) {
            Ok(()) => {
                log::debug!(
                    "MemoryService::munmap: unmapped {} bytes at {:#x}",
                    size,
                    addr
                );
                IpcResponse::ok(0)
            }
            Err(e) => {
                log::warn!("MemoryService::munmap failed: {:?}", e);
                IpcResponse::ok(!0u64)
            }
        }
    }

    /// Handle brk request
    ///
    /// Payload format (8 bytes):
    /// - bytes 0-7: new_brk (u64)
    fn handle_brk(&self, _sender: Tid, request: &IpcRequest) -> IpcResponse {
        let new_brk = if request.payload.len() >= 8 {
            u64::from_le_bytes(request.payload[0..8].try_into().unwrap())
        } else {
            0
        };

        // Get current process's memory set
        let pcb = match process::current_process() {
            Some(pcb) => pcb,
            None => {
                log::warn!("MemoryService::brk: no current process");
                return IpcResponse::ok(!0u64);
            }
        };

        let pcb_lock = pcb.lock();
        let memory_set = Arc::clone(&pcb_lock.memory_set);
        drop(pcb_lock);

        let mut ms = memory_set.lock();

        let current_brk = ms.heap_break();

        // If new_brk is 0, just return current break
        if new_brk == 0 {
            return IpcResponse::ok(current_brk.as_u64());
        }

        let new_brk_addr = VirtAddr::new(new_brk);

        // Validate new break is in user space
        if new_brk >= USER_SPACE_END {
            log::warn!(
                "MemoryService::brk: address {:#x} out of user space",
                new_brk
            );
            return IpcResponse::ok(!0u64);
        }

        // Expand or shrink heap
        if new_brk_addr > current_brk {
            // Expand heap
            match ms.expand_user_heap(new_brk_addr) {
                Ok(()) => IpcResponse::ok(new_brk),
                Err(e) => {
                    log::warn!("MemoryService::brk expand failed: {:?}", e);
                    IpcResponse::ok(!0u64)
                }
            }
        } else {
            // Shrink heap
            match ms.shrink_user_heap(new_brk_addr) {
                Ok(()) => IpcResponse::ok(new_brk),
                Err(e) => {
                    log::warn!("MemoryService::brk shrink failed: {:?}", e);
                    IpcResponse::ok(!0u64)
                }
            }
        }
    }
}

impl Service for MemoryService {
    fn id(&self) -> ServiceId {
        ServiceId::Memory
    }

    fn name(&self) -> &'static str {
        "memory"
    }

    fn handle(&self, sender: Tid, request: &IpcRequest) -> IpcResponse {
        let msg_type = request.header.msg_type;

        match msg_type {
            0 => self.handle_mmap(sender, request),
            1 => self.handle_munmap(sender, request),
            2 => self.handle_brk(sender, request),
            _ => {
                log::warn!("MemoryService: unknown message type {}", msg_type);
                IpcResponse::error(error::ENOSYS)
            }
        }
    }
}
