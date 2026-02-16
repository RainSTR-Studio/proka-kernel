//! Thread Control Block and Scheduler for Proka Kernel
//!
//! This module implements the microkernel-style thread management
//! with support for:
//! - Multi-priority scheduling (0-255)
//! - Blocking states (IPC, Resource, etc.)
//! - Independent address spaces (VSpace)
//! - Capability-based security

use alloc::vec::Vec;
use x86_64::PhysAddr;

/// Thread ID type

pub type Tid = u16;

/// Kernel thread ID (reserved)
pub const KERNEL_TID: Tid = 0;

/// Maximum number of threads supported (u16::MAX, since Tid is u16)
pub const MAX_THREADS: usize = 65535;

/// Number of priority levels (0-255, lower is higher priority)
pub const NUM_PRIORITIES: usize = 256;

/// Thread state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Thread is ready to run
    Runnable,
    /// Thread is currently running
    Running,
    /// Thread is blocked waiting for IPC message
    BlockedIpc {
        sender_tid: Option<Tid>,
        timeout_ms: Option<u64>,
    },
    /// Thread is blocked waiting for resource
    BlockedResource { resource_id: u64 },
    /// Thread has terminated
    Terminated,
}

/// CPU context for context switching
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    /// General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// Instruction pointer
    pub rip: u64,
    /// RFLAGS register
    pub rflags: u64,
    /// Segment selectors
    pub cs: u64,
    pub ss: u64,
    /// FS and GS base (for TLS)
    pub fs_base: u64,
    pub gs_base: u64,
}

/// Thread Control Block (TCB)
pub struct ThreadControlBlock {
    /// Thread ID
    pub tid: Tid,
    /// Current state
    pub state: ThreadState,
    /// Priority (0-255, 0 is highest)
    pub priority: u8,
    /// CPU context for switching
    pub context: Context,
    /// Virtual address space (PML4 physical address)
    pub vspace: Option<PhysAddr>,
    /// Kernel stack top
    pub kernel_stack_top: usize,
    /// User stack top (if user thread)
    pub user_stack_top: Option<usize>,
    /// Entry point
    pub entry_point: usize,
    /// Thread-local storage pointer
    pub tls_ptr: Option<usize>,
    /// Thread name (for debugging)
    pub name: Option<alloc::string::String>,
}

impl ThreadControlBlock {
    /// Create a new kernel thread
    pub fn new_kernel(
        tid: Tid,
        priority: u8,
        entry_point: extern "C" fn() -> !,
        kernel_stack_top: usize,
    ) -> Self {
        let mut context = Context::default();

        // Use the context_switch module to properly initialize context
        super::context_switch::init_context(
            &mut context,
            entry_point as usize,
            kernel_stack_top,
            true, // kernel thread
        );

        Self {
            tid,
            state: ThreadState::Runnable,
            priority,
            context,
            vspace: None,
            kernel_stack_top,
            user_stack_top: None,
            entry_point: entry_point as usize,
            tls_ptr: None,
            name: None,
        }
    }

    /// Create a new user thread with its own address space
    pub fn new_user(
        tid: Tid,
        priority: u8,
        entry_point: usize,
        user_stack_top: usize,
        kernel_stack_top: usize,
        vspace: PhysAddr,
    ) -> Self {
        let mut context = Context::default();
        context.rip = entry_point as u64;
        context.rsp = user_stack_top as u64;
        context.rflags = 0x202; // IF flag set
                                // User mode segment selectors (will be set during context switch)
        context.cs = 0x1B; // User code segment
        context.ss = 0x23; // User data segment

        Self {
            tid,
            state: ThreadState::Runnable,
            priority,
            context,
            vspace: Some(vspace),
            kernel_stack_top,
            user_stack_top: Some(user_stack_top),
            entry_point,
            tls_ptr: None,
            name: None,
        }
    }

    /// Set thread name
    pub fn set_name(&mut self, name: &str) {
        self.name = Some(alloc::string::String::from(name));
    }

    /// Check if thread is runnable
    pub fn is_runnable(&self) -> bool {
        matches!(self.state, ThreadState::Runnable)
    }

    /// Check if thread is blocked
    pub fn is_blocked(&self) -> bool {
        matches!(
            self.state,
            ThreadState::BlockedIpc { .. } | ThreadState::BlockedResource { .. }
        )
    }
}

/// Priority queue for runnable threads
pub struct PriorityQueue {
    /// One queue per priority level
    queues: [Vec<Tid>; NUM_PRIORITIES],
    /// Bitmap to quickly find non-empty queues
    bitmap: [u64; NUM_PRIORITIES / 64],
}

impl PriorityQueue {
    pub const fn new() -> Self {
        const EMPTY_VEC: Vec<Tid> = Vec::new();
        Self {
            queues: [EMPTY_VEC; NUM_PRIORITIES],
            bitmap: [0; NUM_PRIORITIES / 64],
        }
    }

    /// Add a thread to the appropriate priority queue
    pub fn enqueue(&mut self, tid: Tid, priority: u8) {
        let prio = priority as usize;
        self.queues[prio].push(tid);
        // Set bit in bitmap
        let word = prio / 64;
        let bit = prio % 64;
        self.bitmap[word] |= 1 << bit;
    }

    /// Remove and return the highest priority thread
    pub fn dequeue(&mut self) -> Option<Tid> {
        // Find highest priority non-empty queue
        for word_idx in 0..self.bitmap.len() {
            let word = self.bitmap[word_idx];
            if word != 0 {
                // Find first set bit
                let bit = word.trailing_zeros() as usize;
                let priority = word_idx * 64 + bit;

                if let Some(tid) = self.queues[priority].pop() {
                    // If queue is now empty, clear the bit
                    if self.queues[priority].is_empty() {
                        self.bitmap[word_idx] &= !(1 << bit);
                    }
                    return Some(tid);
                }
            }
        }
        None
    }

    /// Remove a specific thread from any queue
    pub fn remove(&mut self, tid: Tid, priority: u8) -> bool {
        let prio = priority as usize;
        let queue = &mut self.queues[prio];

        if let Some(pos) = queue.iter().position(|&t| t == tid) {
            queue.remove(pos);
            // Update bitmap if queue is now empty
            if queue.is_empty() {
                let word = prio / 64;
                let bit = prio % 64;
                self.bitmap[word] &= !(1 << bit);
            }
            return true;
        }
        false
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.bitmap.iter().all(|&w| w == 0)
    }
}

/// Thread scheduler
pub struct Scheduler {
    /// All threads in the system
    threads: Vec<Option<alloc::boxed::Box<ThreadControlBlock>>>,
    /// TID allocator
    next_tid: Tid,
    /// Priority queue for runnable threads
    ready_queue: PriorityQueue,
    /// Currently running thread
    current_tid: Option<Tid>,
    /// Idle thread TID (special kernel thread)
    idle_tid: Option<Tid>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            threads: Vec::new(),
            next_tid: 1, // 0 is reserved for kernel
            ready_queue: PriorityQueue::new(),
            current_tid: None,
            idle_tid: None,
        }
    }

    /// Initialize the scheduler with an idle thread
    pub fn init(&mut self, idle_entry: extern "C" fn() -> !) {
        // Create idle thread (tid 0)
        let idle_stack = allocate_kernel_stack(4096);
        let mut idle_tcb = ThreadControlBlock::new_kernel(
            KERNEL_TID, 255, // Lowest priority
            idle_entry, idle_stack,
        );
        idle_tcb.set_name("idle");

        // Ensure threads vector has space for tid 0
        while self.threads.len() <= KERNEL_TID as usize {
            self.threads.push(None);
        }
        self.threads[KERNEL_TID as usize] = Some(alloc::boxed::Box::new(idle_tcb));
        self.idle_tid = Some(KERNEL_TID);
        self.current_tid = Some(KERNEL_TID);
    }

    /// Create a new kernel thread
    pub fn create_kernel_thread(
        &mut self,
        entry_point: extern "C" fn() -> !,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError> {
        let tid = self.alloc_tid()?;

        let stack = allocate_kernel_stack(8192); // 8KB kernel stack

        let mut tcb = ThreadControlBlock::new_kernel(tid, priority, entry_point, stack);

        if let Some(n) = name {
            tcb.set_name(n);
        }

        // Ensure threads vector has space
        while self.threads.len() <= tid as usize {
            self.threads.push(None);
        }
        self.threads[tid as usize] = Some(alloc::boxed::Box::new(tcb));

        // Add to ready queue
        self.ready_queue.enqueue(tid, priority);

        Ok(tid)
    }

    /// Create a new user thread with its own address space
    pub fn create_user_thread(
        &mut self,
        entry_point: usize,
        user_stack_top: usize,
        kernel_stack_top: usize,
        vspace: PhysAddr,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError> {
        let tid = self.alloc_tid()?;

        let mut tcb = ThreadControlBlock::new_user(
            tid,
            priority,
            entry_point,
            user_stack_top,
            kernel_stack_top,
            vspace,
        );
        if let Some(n) = name {
            tcb.set_name(n);
        }

        while self.threads.len() <= tid as usize {
            self.threads.push(None);
        }
        self.threads[tid as usize] = Some(alloc::boxed::Box::new(tcb));

        self.ready_queue.enqueue(tid, priority);

        Ok(tid)
    }

    /// Terminate a thread
    pub fn terminate_thread(&mut self, tid: Tid) -> Result<(), SchedulerError> {
        let is_current = self.current_tid == Some(tid);

        if let Some(tcb) = self.get_thread_mut(tid) {
            let priority = tcb.priority;
            tcb.state = ThreadState::Terminated;

            // If currently running, will be cleaned up at next schedule
            // Otherwise remove from ready queue
            if !is_current {
                self.ready_queue.remove(tid, priority);
            }

            // TODO: Clean up resources (stack, vspace, etc.)

            Ok(())
        } else {
            Err(SchedulerError::ThreadNotFound)
        }
    }

    /// Block current thread waiting for IPC
    pub fn block_ipc(&mut self, sender_tid: Option<Tid>, timeout_ms: Option<u64>) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                tcb.state = ThreadState::BlockedIpc {
                    sender_tid,
                    timeout_ms,
                };
            }
        }
    }

    /// Unblock a thread (e.g., when IPC message arrives)
    pub fn unblock(&mut self, tid: Tid) -> Result<(), SchedulerError> {
        if let Some(tcb) = self.get_thread_mut(tid) {
            if tcb.is_blocked() {
                let priority = tcb.priority;
                tcb.state = ThreadState::Runnable;
                self.ready_queue.enqueue(tid, priority);
                Ok(())
            } else {
                Err(SchedulerError::NotBlocked)
            }
        } else {
            Err(SchedulerError::ThreadNotFound)
        }
    }

    /// Get the next thread to run
    pub fn schedule(&mut self) -> Option<Tid> {
        // If current thread is still runnable, add it back to queue
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread(current) {
                if tcb.state == ThreadState::Running {
                    let priority = tcb.priority;
                    // Mark as runnable and requeue
                    if let Some(t) = self.get_thread_mut(current) {
                        t.state = ThreadState::Runnable;
                    }
                    self.ready_queue.enqueue(current, priority);
                }
            }
        }

        // Get highest priority runnable thread
        if let Some(next_tid) = self.ready_queue.dequeue() {
            if let Some(tcb) = self.get_thread_mut(next_tid) {
                tcb.state = ThreadState::Running;
            }
            self.current_tid = Some(next_tid);
            Some(next_tid)
        } else {
            // No runnable threads, run idle
            self.current_tid = self.idle_tid;
            self.idle_tid
        }
    }

    /// Get current running thread's TID
    pub fn current_tid(&self) -> Option<Tid> {
        self.current_tid
    }

    /// Get reference to a thread
    pub fn get_thread(&self, tid: Tid) -> Option<&ThreadControlBlock> {
        self.threads.get(tid as usize).and_then(|t| t.as_deref())
    }

    /// Get mutable reference to a thread
    pub fn get_thread_mut(&mut self, tid: Tid) -> Option<&mut ThreadControlBlock> {
        self.threads
            .get_mut(tid as usize)
            .and_then(|t| t.as_deref_mut())
    }

    /// Allocate a new TID
    fn alloc_tid(&mut self) -> Result<Tid, SchedulerError> {
        // Start from 1 since 0 is reserved for idle thread
        let start_tid = if self.next_tid == 0 { 1 } else { self.next_tid };

        // Search for next available TID (u16 range, but 0 is reserved)
        let max_tid = u16::MAX;

        for tid in start_tid..=max_tid {
            // Check if this TID is available
            let idx = tid as usize;
            if idx >= self.threads.len() || self.threads[idx].is_none() {
                self.next_tid = tid.saturating_add(1);
                return Ok(tid);
            }
        }

        // Also check if we can reuse TIDs from 1 to start_tid
        for tid in 1..start_tid {
            let idx = tid as usize;
            if idx >= self.threads.len() || self.threads[idx].is_none() {
                return Ok(tid);
            }
        }

        Err(SchedulerError::MaxThreadsReached)
    }

    /// Yield the current thread
    pub fn yield_current(&mut self) {
        // Force reschedule
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                if tcb.state == ThreadState::Running {
                    tcb.state = ThreadState::Runnable;
                    let priority = tcb.priority;
                    self.ready_queue.enqueue(current, priority);
                }
            }
        }
    }
}

/// Scheduler errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    MaxThreadsReached,
    ThreadNotFound,
    NotBlocked,
    InvalidPriority,
}

/// Allocate a kernel stack
///
/// Returns the virtual address (via HHDM) of the top of the stack
fn allocate_kernel_stack(size: usize) -> usize {
    use crate::memory::frame::FRAME_ALLOCATOR;
    use crate::memory::paging::phys_to_virt;

    let pages = (size + 4095) / 4096;

    let frame = FRAME_ALLOCATOR
        .allocate_contiguous(pages)
        .expect("Failed to allocate kernel stack");

    // Get virtual address via HHDM
    let phys_addr = frame.start_address();
    let virt_addr = phys_to_virt(phys_addr);

    // Stack grows down, so return top address (virtual)
    virt_addr.as_u64() as usize + (pages * 4096)
}

/// Idle thread - runs when no other threads are ready
pub extern "C" fn idle_thread() -> ! {
    loop {
        // Halt CPU until next interrupt
        unsafe { core::arch::asm!("hlt") };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_priority_queue() {
        let mut queue = PriorityQueue::new();

        // Add threads with different priorities
        queue.enqueue(1, 10);
        queue.enqueue(2, 5);
        queue.enqueue(3, 20);
        queue.enqueue(4, 5); // Same priority as 2

        // Should dequeue in priority order (lower number = higher priority)
        assert_eq!(queue.dequeue(), Some(2)); // priority 5
        assert_eq!(queue.dequeue(), Some(4)); // priority 5
        assert_eq!(queue.dequeue(), Some(1)); // priority 10
        assert_eq!(queue.dequeue(), Some(3)); // priority 20
        assert_eq!(queue.dequeue(), None);
    }

    #[test_case]
    fn test_thread_state() {
        let tcb = ThreadControlBlock::new_kernel(1, 10, idle_thread, 0xFFFF800000000000);

        assert_eq!(tcb.tid, 1);
        assert_eq!(tcb.priority, 10);
        assert!(tcb.is_runnable());
        assert!(!tcb.is_blocked());
    }
}
