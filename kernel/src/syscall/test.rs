//! IPC Call Tests
//!
//! This module contains tests for the IPC call mechanism.

use super::*;
use crate::service::{
    self,
    types::{ConsoleMsg, ProcessMsg},
    IpcRequest, ServiceId,
};

/// Test that syscall initialization works
#[test_case]
fn test_syscall_init() {
    // The syscall subsystem should already be initialized by main()
    // Just verify it's enabled
    assert!(msr::is_syscall_enabled());
}

/// Test that service subsystem is initialized
#[test_case]
fn test_service_init() {
    assert!(service::is_initialized());
}

/// Test IPC call to ProcessService::GetPid
#[test_case]
fn test_ipc_get_pid() {
    let request = IpcRequest::empty(ServiceId::Process, ProcessMsg::GetPid as u16);

    let sender = crate::process::scheduler::current_tid().unwrap();
    let response = service::dispatch(sender, &request);

    // Should succeed with a valid PID
    assert_eq!(response.header.status, 0);
    assert!(response.header.retval > 0);
}

/// Test IPC call to ConsoleService::Putc
#[test_case]
fn test_ipc_putc() {
    let payload = alloc::vec![b'T'];
    let request = IpcRequest::new(ServiceId::Console, ConsoleMsg::Putc as u16, payload);

    let sender = crate::process::scheduler::current_tid().unwrap();
    let response = service::dispatch(sender, &request);

    // Should succeed
    assert_eq!(response.header.status, 0);
}

/// Test IPC call to invalid service
#[test_case]
fn test_ipc_invalid_service() {
    let request = IpcRequest::empty(unsafe { core::mem::transmute::<u16, ServiceId>(99u16) }, 0);

    let sender = crate::process::scheduler::current_tid().unwrap();
    let response = service::dispatch(sender, &request);

    // Should fail with service error
    assert_ne!(response.header.status, 0);
}

impl Default for IpcCallArgs {
    fn default() -> Self {
        Self {
            syscall_num: 0,
            service_id: 0,
            payload_ptr: 0,
            _reserved: 0,
            msg_type: 0,
            payload_ptr2: 0,
            payload_size: 0,
            user_rip: 0,
            user_rflags: 0,
            user_rsp: 0,
        }
    }
}
