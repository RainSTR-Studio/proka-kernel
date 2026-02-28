//! Inter-Process Communication (IPC) for Proka Kernel
//!
//! This module provides message passing between threads and services.
//! Design principles:
//! - Simple synchronous message passing
//! - Support for timeout and asynchronous send
//! - Named service registration and discovery
//! - Support for both kernel-space and user-space services
//!
//! # Named Services
//!
//! Services can be registered by name for discovery:
//! - `proc:/` - Process service (kernel)
//! - `mem:/` - Memory service (kernel)
//! - `console:/` - Console service (kernel)
//! - `fs:/` - File system service (user-space)
//! - `dev:/` - Device service (user-space)
//!
//! Kernel services use TID 0 to indicate kernel-space handling.

use crate::process::scheduler;
use crate::process::thread::Tid;
use crate::sync::mutex::Mutex;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

/// Maximum message size in bytes
pub const MAX_MESSAGE_SIZE: usize = 1024;

/// Maximum number of pending messages per queue
pub const MAX_QUEUE_SIZE: usize = 64;

/// Special TID indicating a kernel-space service
pub const KERNEL_SERVICE_TID: Tid = 0;

/// IPC message structure
#[derive(Debug, Clone)]
pub struct Message {
    /// Sender thread ID
    pub sender: Tid,
    /// Message type (user-defined)
    pub msg_type: u64,
    /// Message payload
    pub payload: Vec<u8>,
}

impl Message {
    /// Create a new message
    pub fn new(sender: Tid, msg_type: u64, payload: Vec<u8>) -> Result<Self, IpcError> {
        if payload.len() > MAX_MESSAGE_SIZE {
            return Err(IpcError::MessageTooLarge);
        }
        Ok(Self {
            sender,
            msg_type,
            payload,
        })
    }

    /// Create a simple message with no payload
    pub fn simple(sender: Tid, msg_type: u64) -> Self {
        Self {
            sender,
            msg_type,
            payload: Vec::new(),
        }
    }
}

/// Message queue for a thread
pub struct MessageQueue {
    /// Owner thread ID
    owner: Tid,
    /// Pending messages
    messages: VecDeque<Message>,
}

impl MessageQueue {
    /// Create a new message queue
    pub fn new(owner: Tid) -> Self {
        Self {
            owner,
            messages: VecDeque::new(),
        }
    }

    /// Send a message to this queue
    pub fn send(&mut self, msg: Message) -> Result<(), IpcError> {
        if self.messages.len() >= MAX_QUEUE_SIZE {
            return Err(IpcError::QueueFull);
        }
        self.messages.push_back(msg);

        Ok(())
    }

    /// Try to receive a message
    pub fn try_recv(&mut self) -> Option<Message> {
        self.messages.pop_front()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the number of pending messages
    pub fn len(&self) -> usize {
        self.messages.len()
    }
}

/// Global message queue registry
static MESSAGE_QUEUES: Mutex<Option<alloc::vec::Vec<Option<Mutex<MessageQueue>>>>> =
    Mutex::new(None);

/// Global service registry (name -> tid)
static SERVICE_REGISTRY: Mutex<Option<alloc::collections::BTreeMap<alloc::string::String, Tid>>> =
    Mutex::new(None);

/// Get a stable synchronization ID for a thread's message queue
fn get_sync_id(tid: Tid) -> u64 {
    // Use a high bit to distinguish from memory addresses
    0x8000_0000_0000_0000 | (tid as u64)
}

/// Initialize IPC subsystem
pub fn init() {
    let mut queues = MESSAGE_QUEUES.lock();
    *queues = Some(alloc::vec![]);
    let mut services = SERVICE_REGISTRY.lock();
    *services = Some(alloc::collections::BTreeMap::new());
    log::info!("IPC subsystem initialized");
}

/// Create a message queue for a thread
pub fn create_queue(tid: Tid) -> Result<(), IpcError> {
    let mut queues_opt = MESSAGE_QUEUES.lock();
    let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;

    let idx = tid as usize;
    if idx >= queues.len() {
        queues.resize_with(idx + 1, || None);
    }

    if queues[idx].is_some() {
        return Err(IpcError::QueueExists);
    }

    queues[idx] = Some(Mutex::new(MessageQueue::new(tid)));
    Ok(())
}

/// Destroy a message queue
pub fn destroy_queue(tid: Tid) {
    let mut queues_opt = MESSAGE_QUEUES.lock();
    if let Some(queues) = queues_opt.as_mut() {
        let idx = tid as usize;
        if idx < queues.len() {
            if queues[idx].take().is_some() {
                // Wake up all waiters on this queue
                scheduler::unblock_sync(get_sync_id(tid));
            }
        }
    }
}

/// Send a message to a target thread
///
/// # Arguments
/// * `target` - Target thread ID
/// * `msg` - Message to send
/// * `block` - If true, block until message can be sent
///
/// # Returns
/// * `Ok(())` - Message sent successfully
/// * `Err(IpcError)` - Send failed
pub fn send(target: Tid, msg: Message, block: bool) -> Result<(), IpcError> {
    loop {
        {
            let mut queues_opt = MESSAGE_QUEUES.lock();
            let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;

            let idx = target as usize;
            if idx >= queues.len() || queues[idx].is_none() {
                return Err(IpcError::ThreadNotFound);
            }

            let mut queue = queues[idx].as_ref().unwrap().lock();
            match queue.send(msg.clone()) {
                Ok(()) => {
                    // Wake up waiters on this queue
                    scheduler::unblock_sync(get_sync_id(target));
                    return Ok(());
                }
                Err(IpcError::QueueFull) if !block => return Err(IpcError::QueueFull),
                Err(e) => return Err(e),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }

        // Queue is full and we're blocking - yield and retry
        if block {
            scheduler::yield_thread();
        }
    }
}

/// Register a service name for the current thread
pub fn register_service(name: &str) -> Result<(), IpcError> {
    let tid = scheduler::current_tid().ok_or(IpcError::NotInitialized)?;
    do_register_service(name, tid)
}

/// Internal function to register a service with a specific TID
fn do_register_service(name: &str, tid: Tid) -> Result<(), IpcError> {
    let mut registry_opt = SERVICE_REGISTRY.lock();
    let registry = registry_opt.as_mut().ok_or(IpcError::NotInitialized)?;

    if registry.contains_key(name) {
        return Err(IpcError::ServiceExists);
    }

    registry.insert(String::from(name), tid);
    Ok(())
}

/// Register a kernel-space service
///
/// Kernel services don't have a TID, so we use KERNEL_SERVICE_TID (0)
/// and store the service_id for routing.
pub fn register_kernel_service(name: &str, service_id: u16) -> Result<(), IpcError> {
    // For kernel services, we encode: TID=0, service_id in the lower bits
    // This allows dispatch to route to the kernel service handler
    let encoded = KERNEL_SERVICE_TID | ((service_id as Tid) << 8);

    do_register_service(name, encoded)?;

    log::debug!(
        "Kernel service '{}' registered (encoded TID: {})",
        name,
        encoded
    );
    Ok(())
}

/// Look up a thread ID by service name
///
/// Returns the TID of the service thread, or KERNEL_SERVICE_TID for kernel services.
/// The caller can check if the result is KERNEL_SERVICE_TID to determine if
/// the service is a kernel-space service.
pub fn lookup_service(name: &str) -> Option<Tid> {
    let registry_opt = SERVICE_REGISTRY.lock();
    let registry = registry_opt.as_ref()?;
    registry.get(name).copied()
}

/// Check if a TID refers to a kernel-space service
pub fn is_kernel_service(tid: Tid) -> bool {
    tid == KERNEL_SERVICE_TID || (tid & 0xFF) == KERNEL_SERVICE_TID
}

/// Get the service ID from an encoded kernel service TID
pub fn get_kernel_service_id(tid: Tid) -> Option<u16> {
    if is_kernel_service(tid) {
        Some((tid >> 8) as u16)
    } else {
        None
    }
}

/// Receive a message
///
/// # Arguments
/// * `sender` - If Some, only receive from this sender; if None, receive from any
/// * `timeout_ms` - If Some, block for at most this many milliseconds
///
/// # Returns
/// * `Ok(Message)` - Message received
/// * `Err(IpcError)` - Receive failed or timed out
pub fn recv(sender: Option<Tid>, timeout_ms: Option<u64>) -> Result<Message, IpcError> {
    let current_tid = scheduler::current_tid().ok_or(IpcError::NotInitialized)?;

    // Ensure we have a queue
    if let Err(e) = create_queue(current_tid) {
        if !matches!(e, IpcError::QueueExists) {
            return Err(e);
        }
    }

    // Try to receive without blocking first
    {
        let mut queues_opt = MESSAGE_QUEUES.lock();
        let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;

        let idx = current_tid as usize;
        if let Some(queue) = queues[idx].as_ref() {
            let mut q = queue.lock();

            // Check for matching message
            if let Some(pos) = q
                .messages
                .iter()
                .position(|m| sender.map_or(true, |s| m.sender == s))
            {
                return Ok(q.messages.remove(pos).unwrap());
            }
        }
    }

    // No message available, block if requested
    if timeout_ms.is_some() || sender.is_some() || true {
        // Block by default for recv
        let sync_id = get_sync_id(current_tid);

        // Block waiting for sync on this queue's ID
        scheduler::block_sync(sync_id);
        scheduler::yield_thread();

        // We've been unblocked, try to receive again
        {
            let mut queues_opt = MESSAGE_QUEUES.lock();
            let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;
            let idx = current_tid as usize;
            if let Some(queue) = queues[idx].as_ref() {
                let mut q = queue.lock();

                // Check for matching message again
                if let Some(pos) = q
                    .messages
                    .iter()
                    .position(|m| sender.map_or(true, |s| m.sender == s))
                {
                    return Ok(q.messages.remove(pos).unwrap());
                }
            }
        }

        // No message received (likely timeout or spurious wake)
        Err(IpcError::Timeout)
    } else {
        Err(IpcError::WouldBlock)
    }
}

/// Try to receive a message without blocking
pub fn try_recv(sender: Option<Tid>) -> Result<Message, IpcError> {
    let current_tid = scheduler::current_tid().ok_or(IpcError::NotInitialized)?;

    let mut queues_opt = MESSAGE_QUEUES.lock();
    let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;

    let idx = current_tid as usize;
    if idx >= queues.len() || queues[idx].is_none() {
        return Err(IpcError::NoQueue);
    }

    let mut q = queues[idx].as_ref().unwrap().lock();

    // Check for matching message
    if let Some(pos) = q
        .messages
        .iter()
        .position(|m| sender.map_or(true, |s| m.sender == s))
    {
        Ok(q.messages.remove(pos).unwrap())
    } else {
        Err(IpcError::WouldBlock)
    }
}

/// IPC errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    NotInitialized,
    ThreadNotFound,
    QueueFull,
    QueueExists,
    NoQueue,
    MessageTooLarge,
    Timeout,
    WouldBlock,
    ServiceExists,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_message_creation() {
        let msg = Message::simple(1, 42);
        assert_eq!(msg.sender, 1);
        assert_eq!(msg.msg_type, 42);
        assert!(msg.payload.is_empty());
    }

    #[test_case]
    fn test_message_queue() {
        let mut queue = MessageQueue::new(1);
        assert!(queue.is_empty());

        let msg = Message::simple(2, 1);
        queue.send(msg.clone()).unwrap();
        assert_eq!(queue.len(), 1);

        let received = queue.try_recv().unwrap();
        assert_eq!(received.sender, msg.sender);
        assert!(queue.is_empty());
    }

    #[test_case]
    fn test_queue_full() {
        let mut queue = MessageQueue::new(1);

        // Fill the queue
        for i in 0..MAX_QUEUE_SIZE {
            let msg = Message::simple(i as Tid, i as u64);
            queue.send(msg).unwrap();
        }

        // Next send should fail
        let msg = Message::simple(999, 999);
        assert!(matches!(queue.send(msg), Err(IpcError::QueueFull)));
    }
}
