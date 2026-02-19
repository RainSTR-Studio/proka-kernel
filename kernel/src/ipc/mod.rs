//! Inter-Process Communication (IPC) for Proka Kernel
//!
//! This module provides message passing between threads.
//! Design principles:
//! - Simple synchronous message passing
//! - Support for timeout and asynchronous send
//! - Efficient for kernel-level communication

use crate::process::scheduler;
use crate::process::thread::Tid;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;

/// Maximum message size in bytes
pub const MAX_MESSAGE_SIZE: usize = 1024;

/// Maximum number of pending messages per queue
pub const MAX_QUEUE_SIZE: usize = 64;

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
    /// Threads waiting to receive from this queue
    waiters: Vec<Tid>,
}

impl MessageQueue {
    /// Create a new message queue
    pub fn new(owner: Tid) -> Self {
        Self {
            owner,
            messages: VecDeque::new(),
            waiters: Vec::new(),
        }
    }

    /// Send a message to this queue
    pub fn send(&mut self, msg: Message) -> Result<(), IpcError> {
        if self.messages.len() >= MAX_QUEUE_SIZE {
            return Err(IpcError::QueueFull);
        }
        self.messages.push_back(msg);

        // Wake up a waiter if any
        if let Some(waiter) = self.waiters.pop() {
            let _ = scheduler::unblock(waiter);
        }

        Ok(())
    }

    /// Try to receive a message
    pub fn try_recv(&mut self) -> Option<Message> {
        self.messages.pop_front()
    }

    /// Add a waiter to this queue
    pub fn add_waiter(&mut self, tid: Tid) {
        if !self.waiters.contains(&tid) {
            self.waiters.push(tid);
        }
    }

    /// Remove a waiter from this queue
    pub fn remove_waiter(&mut self, tid: Tid) {
        self.waiters.retain(|&t| t != tid);
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
            // Wake up all waiters before destroying
            if let Some(queue) = queues[idx].take() {
                let q = queue.lock();
                for waiter in &q.waiters {
                    let _ = scheduler::unblock(*waiter);
                }
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
                Ok(()) => return Ok(()),
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
    let mut registry_opt = SERVICE_REGISTRY.lock();
    let registry = registry_opt.as_mut().ok_or(IpcError::NotInitialized)?;

    if registry.contains_key(name) {
        return Err(IpcError::ServiceExists);
    }

    registry.insert(alloc::string::String::from(name), tid);
    Ok(())
}

/// Look up a thread ID by service name
pub fn lookup_service(name: &str) -> Option<Tid> {
    let registry_opt = SERVICE_REGISTRY.lock();
    let registry = registry_opt.as_ref()?;
    registry.get(name).cloned()
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
    if timeout_ms.is_some() || sender.is_some() {
        // Add ourselves to waiters
        {
            let mut queues_opt = MESSAGE_QUEUES.lock();
            let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;
            let idx = current_tid as usize;
            if let Some(queue) = queues[idx].as_ref() {
                queue.lock().add_waiter(current_tid);
            }
        }

        // Block waiting for IPC
        scheduler::block_ipc(sender, timeout_ms);

        // We've been unblocked, try to receive again
        // Note: In a real implementation, we'd need to handle timeout
        // by checking the current time against the deadline

        {
            let mut queues_opt = MESSAGE_QUEUES.lock();
            let queues = queues_opt.as_mut().ok_or(IpcError::NotInitialized)?;
            let idx = current_tid as usize;
            if let Some(queue) = queues[idx].as_ref() {
                let mut q = queue.lock();
                q.remove_waiter(current_tid);

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

        // No message received (likely timeout)
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
