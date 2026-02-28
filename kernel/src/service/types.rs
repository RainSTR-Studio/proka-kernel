//! IPC Service Types
//!
//! Defines the unified IPC message format for all kernel services.
//! All system operations now go through IPC messages to service processes.

use crate::process::thread::Tid;

/// Service IDs - identifies which service handles a request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ServiceId {
    /// Process manager service (exit, spawn, get_pid)
    Process = 0,
    /// Memory manager service (mmap, munmap, brk)
    Memory = 1,
    /// Console/IO service (putc, getc)
    Console = 2,
    /// File system service
    FileSystem = 3,
    /// Device manager service
    Device = 4,
}

impl ServiceId {
    /// Convert from u16 to ServiceId
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(ServiceId::Process),
            1 => Some(ServiceId::Memory),
            2 => Some(ServiceId::Console),
            3 => Some(ServiceId::FileSystem),
            4 => Some(ServiceId::Device),
            _ => None,
        }
    }

    /// Get the canonical service name (for service registry lookup)
    pub fn service_name(&self) -> &'static str {
        match self {
            ServiceId::Process => "proc:/",
            ServiceId::Memory => "mem:/",
            ServiceId::Console => "console:/",
            ServiceId::FileSystem => "fs:/",
            ServiceId::Device => "dev:/",
        }
    }
}

/// Process service message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProcessMsg {
    /// Exit current process
    Exit = 0,
    /// Get current process ID
    GetPid = 1,
    /// Spawn a new process
    Spawn = 2,
    /// Wait for child process
    Wait = 3,
}

/// Memory service message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MemoryMsg {
    /// Map memory region
    Mmap = 0,
    /// Unmap memory region
    Munmap = 1,
    /// Adjust heap break
    Brk = 2,
}

/// Console service message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ConsoleMsg {
    /// Output a character
    Putc = 0,
    /// Input a character
    Getc = 1,
    /// Write string
    Write = 2,
    /// Read string
    Read = 3,
}

/// IPC Request header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IpcRequestHeader {
    /// Target service ID
    pub service: u16,
    /// Message type within the service
    pub msg_type: u16,
    /// Flags (reserved for future use)
    pub flags: u32,
    /// Payload size in bytes
    pub payload_size: u64,
}

/// IPC Request - sent from user space to kernel services
#[derive(Debug, Clone)]
pub struct IpcRequest {
    /// Request header
    pub header: IpcRequestHeader,
    /// Payload data (up to 1016 bytes to fit in 1KB message)
    pub payload: alloc::vec::Vec<u8>,
}

impl IpcRequest {
    /// Create a new IPC request
    pub fn new(service: ServiceId, msg_type: u16, payload: alloc::vec::Vec<u8>) -> Self {
        Self {
            header: IpcRequestHeader {
                service: service as u16,
                msg_type,
                flags: 0,
                payload_size: payload.len() as u64,
            },
            payload,
        }
    }

    /// Create a request with no payload
    pub fn empty(service: ServiceId, msg_type: u16) -> Self {
        Self::new(service, msg_type, alloc::vec::Vec::new())
    }
}

/// IPC Response header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IpcResponseHeader {
    /// Status code (0 = success, non-zero = error)
    pub status: i64,
    /// Return value
    pub retval: u64,
    /// Payload size in bytes
    pub payload_size: u64,
}

/// IPC Response - sent from kernel services back to user space
#[derive(Debug, Clone)]
pub struct IpcResponse {
    /// Response header
    pub header: IpcResponseHeader,
    /// Payload data
    pub payload: alloc::vec::Vec<u8>,
}

impl IpcResponse {
    /// Create a successful response with a return value
    pub fn ok(retval: u64) -> Self {
        Self {
            header: IpcResponseHeader {
                status: 0,
                retval,
                payload_size: 0,
            },
            payload: alloc::vec::Vec::new(),
        }
    }

    /// Create a successful response with payload
    pub fn ok_with_payload(retval: u64, payload: alloc::vec::Vec<u8>) -> Self {
        let size = payload.len() as u64;
        Self {
            header: IpcResponseHeader {
                status: 0,
                retval,
                payload_size: size,
            },
            payload,
        }
    }

    /// Create an error response
    pub fn error(code: i64) -> Self {
        Self {
            header: IpcResponseHeader {
                status: code,
                retval: 0,
                payload_size: 0,
            },
            payload: alloc::vec::Vec::new(),
        }
    }
}

/// Error codes for IPC responses
pub mod error {
    /// Invalid argument
    pub const EINVAL: i64 = 22;
    /// Function not implemented
    pub const ENOSYS: i64 = 38;
    /// No such process
    pub const ESRCH: i64 = 3;
    /// Out of memory
    pub const ENOMEM: i64 = 12;
    /// Invalid service
    pub const ESRV: i64 = 100;
    /// Service unavailable
    pub const ESRVUNAVAIL: i64 = 101;
}

/// Service trait - implemented by all kernel services
pub trait Service: Send {
    /// Get the service ID
    fn id(&self) -> ServiceId;

    /// Get the service name
    fn name(&self) -> &'static str;

    /// Handle an IPC request
    fn handle(&self, sender: Tid, request: &IpcRequest) -> IpcResponse;

    /// Initialize the service (called once at boot)
    fn init(&mut self) {}
}
