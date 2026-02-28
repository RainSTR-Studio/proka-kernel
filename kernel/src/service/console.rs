//! Console Service
//!
//! Handles console/IO operations:
//! - putc: Output a character
//! - getc: Input a character (future)
//! - write: Write string
//! - read: Read string (future)

use super::error;
use super::{IpcRequest, IpcResponse, Service, ServiceId};

use crate::process::thread::Tid;

/// Console service implementation
pub struct ConsoleService;

impl ConsoleService {
    /// Create a new console service
    pub fn new() -> Self {
        Self
    }

    /// Handle putc request
    ///
    /// Payload format (1 byte):
    /// - byte 0: character to output
    fn handle_putc(&self, _sender: Tid, request: &IpcRequest) -> IpcResponse {
        if request.payload.is_empty() {
            return IpcResponse::error(error::EINVAL);
        }

        let c = request.payload[0] as char;
        crate::serial_print!("{}", c);
        IpcResponse::ok(0)
    }

    /// Handle getc request (not yet implemented)
    fn handle_getc(&self, _sender: Tid, _request: &IpcRequest) -> IpcResponse {
        log::warn!("ConsoleService::getc not implemented");
        IpcResponse::error(error::ENOSYS)
    }

    /// Handle write request
    ///
    /// Payload format:
    /// - bytes: string data
    fn handle_write(&self, _sender: Tid, request: &IpcRequest) -> IpcResponse {
        // Output each byte as a character
        for &byte in &request.payload {
            crate::serial_print!("{}", byte as char);
        }
        IpcResponse::ok(request.payload.len() as u64)
    }

    /// Handle read request (not yet implemented)
    fn handle_read(&self, _sender: Tid, _request: &IpcRequest) -> IpcResponse {
        log::warn!("ConsoleService::read not implemented");
        IpcResponse::error(error::ENOSYS)
    }
}

impl Service for ConsoleService {
    fn id(&self) -> ServiceId {
        ServiceId::Console
    }

    fn name(&self) -> &'static str {
        "console"
    }

    fn handle(&self, sender: Tid, request: &IpcRequest) -> IpcResponse {
        let msg_type = request.header.msg_type;

        match msg_type {
            0 => self.handle_putc(sender, request),
            1 => self.handle_getc(sender, request),
            2 => self.handle_write(sender, request),
            3 => self.handle_read(sender, request),
            _ => {
                log::warn!("ConsoleService: unknown message type {}", msg_type);
                IpcResponse::error(error::ENOSYS)
            }
        }
    }
}
