//! Kernel Services
//!
//! This module implements the microkernel service architecture.
//! All system operations are handled by services that communicate via IPC.
//!
//! # Architecture
//!
//! ```text
//! User Process
//!     |
//!     | ipc_call(syscall)
//!     v
//! +------------------+
//! | IPC Dispatcher   |  <-- Single syscall entry point
//! +------------------+
//!     |
//!     | Route by ServiceId or Name
//!     v
//! +------------------+
//! | Service Registry |
//! +------------------+
//!     |
//!     +---> proc:/     (ProcessService) - 进程管理
//!     +---> mem:/      (MemoryService)  - 内存管理
//!     +---> console:/  (ConsoleService) - 控制台
//!     +---> fs:/       (FileService)    - 文件系统 [用户态服务]
//!     +---> dev:/      (DeviceService)  - 设备管理 [用户态服务]
//! ```
//!
//! # 命名服务
//!
//! 服务通过名称注册，格式为 `<type>:/`：
//! - `proc:/` - 进程服务
//! - `mem:/` - 内存服务
//! - `console:/` - 控制台服务
//! - `fs:/` - 文件系统服务
//! - `dev:/` - 设备服务
//!
//! 用户态服务可以通过 `ipc::register_service()` 注册自己的服务名。

pub mod console;
pub mod memory;
pub mod process;
pub mod types;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub use types::*;

use crate::ipc;
use crate::process::thread::Tid;
use crate::sync::mutex::Mutex as ProkaMutex;

/// Maximum number of services
const MAX_SERVICES: usize = 16;

/// Kernel service entry (wraps Service trait object)
struct KernelService {
    /// The service implementation
    service: Arc<Mutex<Box<dyn Service>>>,
}

/// Service registry - holds all registered services
static SERVICE_REGISTRY: ProkaMutex<Vec<Option<Arc<Mutex<Box<dyn Service>>>>>> =
    ProkaMutex::new(Vec::new());

/// Name to ServiceId mapping for named service lookup
static SERVICE_NAME_MAP: ProkaMutex<Option<BTreeMap<String, ServiceId>>> = ProkaMutex::new(None);

/// Initialize the service subsystem
pub fn init() {
    let mut registry = SERVICE_REGISTRY.lock();
    registry.resize(MAX_SERVICES, None);

    // Initialize name map (don't hold the lock across service registration)
    {
        let mut name_map = SERVICE_NAME_MAP.lock();
        *name_map = Some(BTreeMap::new());
    }

    // Register core kernel services
    register_kernel_service_locked(&mut registry, Box::new(process::ProcessService::new()));
    register_kernel_service_locked(&mut registry, Box::new(memory::MemoryService::new()));
    register_kernel_service_locked(&mut registry, Box::new(console::ConsoleService::new()));

    log::info!(
        "Service subsystem initialized with {} services",
        registry.iter().filter(|s| s.is_some()).count()
    );
}

/// Register a kernel service (internal, assumes lock held)
fn register_kernel_service_locked(
    registry: &mut Vec<Option<Arc<Mutex<Box<dyn Service>>>>>,
    mut service: Box<dyn Service>,
) {
    let id = service.id() as usize;
    if id >= registry.len() {
        registry.resize(id + 1, None);
    }

    // Initialize the service
    service.init();

    // Get service name before moving
    let service_name = service.name();

    // Wrap in Arc<Mutex> for thread-safe access
    registry[id] = Some(Arc::new(Mutex::new(service)));

    // Register name mapping
    if let Some(name_map) = SERVICE_NAME_MAP.lock().as_mut() {
        let service_id = ServiceId::from_u16(id as u16).unwrap();
        name_map.insert(String::from(service_id.service_name()), service_id);
    }

    // Also register in IPC module for user-space service discovery
    // Note: Kernel services use a special TID (0) to indicate kernel-space
    let _ = ipc::register_kernel_service(service_name, id as u16);
}

/// Register a user-space service
///
/// This is called by user-space service processes to register themselves.
/// Returns Ok(()) on success, Err(()) if service already exists.
pub fn register_user_service(name: &str, tid: Tid) -> Result<ServiceId, ()> {
    // Parse service name to determine ServiceId
    let service_id = match name {
        "fs:/" => ServiceId::FileSystem,
        "dev:/" => ServiceId::Device,
        _ => return Err(()),
    };

    // Register in IPC for message routing
    ipc::register_service(name).map_err(|_| ())?;

    // Update name map
    if let Some(name_map) = SERVICE_NAME_MAP.lock().as_mut() {
        name_map.insert(String::from(name), service_id);
    }

    log::info!("User-space service '{}' registered (TID: {})", name, tid);
    Ok(service_id)
}

/// Get a service by ID
pub fn get_service(id: ServiceId) -> Option<Arc<Mutex<Box<dyn Service>>>> {
    let registry = SERVICE_REGISTRY.lock();
    let idx = id as usize;
    if idx < registry.len() {
        registry[idx].clone()
    } else {
        None
    }
}

/// Lookup a service by name
///
/// Returns the ServiceId if found, None otherwise.
/// Works for both kernel and user-space services.
pub fn lookup_service(name: &str) -> Option<ServiceId> {
    let name_map = SERVICE_NAME_MAP.lock();
    name_map.as_ref()?.get(name).copied()
}

/// Dispatch an IPC request to the appropriate service
pub fn dispatch(sender: Tid, request: &IpcRequest) -> IpcResponse {
    let service_id = match ServiceId::from_u16(request.header.service) {
        Some(id) => id,
        None => {
            log::warn!("Invalid service ID: {}", request.header.service);
            return IpcResponse::error(error::ESRV);
        }
    };

    let service = match get_service(service_id) {
        Some(s) => s,
        None => {
            log::warn!("Service not registered: {:?}", service_id);
            return IpcResponse::error(error::ESRVUNAVAIL);
        }
    };

    let svc = service.lock();
    svc.handle(sender, request)
}

/// Check if the service subsystem is initialized
pub fn is_initialized() -> bool {
    let registry = SERVICE_REGISTRY.lock();
    !registry.is_empty()
}
