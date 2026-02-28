//! Process Service
//!
//! Handles process-related operations:
//! - Exit current process
//! - Get process ID
//! - Spawn new processes (future)
//! - Wait for child processes (future)

use super::error;
use super::{IpcRequest, IpcResponse, Service, ServiceId};

use crate::process::thread::Tid;
use crate::process::{self, scheduler};

/// Process service implementation
pub struct ProcessService;

impl ProcessService {
    /// Create a new process service
    pub fn new() -> Self {
        Self
    }

    /// Handle exit request
    fn handle_exit(&self, sender: Tid, request: &IpcRequest) -> IpcResponse {
        // Extract exit code from payload (4 bytes, little-endian)
        let exit_code = if request.payload.len() >= 4 {
            i32::from_le_bytes([
                request.payload[0],
                request.payload[1],
                request.payload[2],
                request.payload[3],
            ])
        } else {
            0
        };

        log::debug!(
            "ProcessService::exit(code={}) from TID {}",
            exit_code,
            sender
        );

        // Terminate the current thread
        scheduler::terminate_self();
    }

    /// Handle get_pid request
    fn handle_get_pid(&self, _sender: Tid, _request: &IpcRequest) -> IpcResponse {
        match process::current_pid() {
            Some(pid) => IpcResponse::ok(pid as u64),
            None => {
                log::warn!("ProcessService::get_pid: no current process");
                IpcResponse::error(error::ESRCH)
            }
        }
    }

    /// Handle spawn request (not yet implemented)
    fn handle_spawn(&self, _sender: Tid, _request: &IpcRequest) -> IpcResponse {
        log::warn!("ProcessService::spawn not implemented");
        IpcResponse::error(error::ENOSYS)
    }

    /// Handle wait request (not yet implemented)
    fn handle_wait(&self, _sender: Tid, _request: &IpcRequest) -> IpcResponse {
        log::warn!("ProcessService::wait not implemented");
        IpcResponse::error(error::ENOSYS)
    }
}

impl Service for ProcessService {
    fn id(&self) -> ServiceId {
        ServiceId::Process
    }

    fn name(&self) -> &'static str {
        "process"
    }

    fn handle(&self, sender: Tid, request: &IpcRequest) -> IpcResponse {
        let msg_type = request.header.msg_type;

        match msg_type {
            0 => self.handle_exit(sender, request),
            1 => self.handle_get_pid(sender, request),
            2 => self.handle_spawn(sender, request),
            3 => self.handle_wait(sender, request),
            _ => {
                log::warn!("ProcessService: unknown message type {}", msg_type);
                IpcResponse::error(error::ENOSYS)
            }
        }
    }
}
