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
#[derive(Debug)]
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
    /// Kernel stack top (Virtual)
    pub kernel_stack_top: usize,
    /// Kernel stack physical base (for deallocation)
    pub kernel_stack_phys: PhysAddr,
    /// Kernel stack size in pages
    pub kernel_stack_pages: usize,
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
        stack_info: (usize, PhysAddr, usize),
    ) -> Self {
        let (kernel_stack_top, kernel_stack_phys, kernel_stack_pages) = stack_info;
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
            kernel_stack_phys,
            kernel_stack_pages,
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
        stack_info: (usize, PhysAddr, usize),
        vspace: PhysAddr,
    ) -> Self {
        let (kernel_stack_top, kernel_stack_phys, kernel_stack_pages) = stack_info;
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
            kernel_stack_phys,
            kernel_stack_pages,
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
    /// Counter for fairness
    dequeue_count: u32,
}

impl PriorityQueue {
    pub const fn new() -> Self {
        const EMPTY_VEC: Vec<Tid> = Vec::new();
        Self {
            queues: [EMPTY_VEC; NUM_PRIORITIES],
            bitmap: [0; NUM_PRIORITIES / 64],
            dequeue_count: 0,
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
        self.dequeue_count = self.dequeue_count.wrapping_add(1);

        // Every 16th dequeue, try to pick from lower priorities for fairness
        if self.dequeue_count % 16 == 0 {
            // Search from lowest to highest
            for word_idx in (0..self.bitmap.len()).rev() {
                let mut word = self.bitmap[word_idx];
                while word != 0 {
                    // Find last set bit (most significant)
                    let bit = 63 - word.leading_zeros() as usize;
                    let priority = word_idx * 64 + bit;

                    if !self.queues[priority].is_empty() {
                        let tid = self.queues[priority].remove(0);
                        if self.queues[priority].is_empty() {
                            self.bitmap[word_idx] &= !(1 << bit);
                        }
                        return Some(tid);
                    }
                    word &= !(1 << bit);
                }
            }
        }

        // Normal path: Find highest priority non-empty queue
        for word_idx in 0..self.bitmap.len() {
            let mut word = self.bitmap[word_idx];
            while word != 0 {
                // Find first set bit (least significant)
                let bit = word.trailing_zeros() as usize;
                let priority = word_idx * 64 + bit;

                if !self.queues[priority].is_empty() {
                    let tid = self.queues[priority].remove(0);
                    // If queue is now empty, clear the bit in the ACTUAL bitmap
                    if self.queues[priority].is_empty() {
                        self.bitmap[word_idx] &= !(1 << bit);
                    }
                    return Some(tid);
                } else {
                    // Ghost bit - clear it and continue searching this word
                    self.bitmap[word_idx] &= !(1 << bit);
                    word &= !(1 << bit);
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
    /// Threads waiting to be reaped
    zombie_queue: Vec<Tid>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            threads: Vec::new(),
            next_tid: 1, // 0 is reserved for kernel
            ready_queue: PriorityQueue::new(),
            current_tid: None,
            idle_tid: None,
            zombie_queue: Vec::new(),
        }
    }

    /// Initialize the scheduler with an idle thread
    pub fn init(&mut self, idle_entry: extern "C" fn() -> !) {
        // Create idle thread (tid 0)
        let stack_info = allocate_kernel_stack(4096);
        let mut idle_tcb = ThreadControlBlock::new_kernel(
            KERNEL_TID,
            0, // Set initial priority to 0 (highest) to avoid starvation during init
            idle_entry, stack_info,
        );
        idle_tcb.set_name("idle");
        // The boot thread is currently running, so mark TID 0 as Running
        idle_tcb.state = ThreadState::Running;

        // Ensure threads vector has space for tid 0
        while self.threads.len() <= KERNEL_TID as usize {
            self.threads.push(None);
        }
        self.threads[KERNEL_TID as usize] = Some(alloc::boxed::Box::new(idle_tcb));
        self.idle_tid = Some(KERNEL_TID);
        self.current_tid = Some(KERNEL_TID);
    }

    /// Change the priority of a thread
    pub fn set_priority(&mut self, tid: Tid, new_priority: u8) -> Result<(), SchedulerError> {
        if let Some(tcb) = self.get_thread_mut(tid) {
            let old_priority = tcb.priority;
            if old_priority == new_priority {
                return Ok(());
            }

            tcb.priority = new_priority;

            // If the thread is in the ready queue, we need to move it to the new priority queue
            if tcb.state == ThreadState::Runnable {
                self.ready_queue.remove(tid, old_priority);
                self.ready_queue.enqueue(tid, new_priority);
            }
            Ok(())
        } else {
            Err(SchedulerError::ThreadNotFound)
        }
    }

    /// Create a new kernel thread
    pub fn create_kernel_thread(
        &mut self,
        entry_point: extern "C" fn() -> !,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError> {
        let tid = self.alloc_tid()?;

        let stack_info = allocate_kernel_stack(8192); // 8KB kernel stack

        let mut tcb = ThreadControlBlock::new_kernel(tid, priority, entry_point, stack_info);

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
        kernel_stack_size: usize,
        vspace: PhysAddr,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError> {
        let tid = self.alloc_tid()?;
        let stack_info = allocate_kernel_stack(kernel_stack_size);

        let mut tcb = ThreadControlBlock::new_user(
            tid,
            priority,
            entry_point,
            user_stack_top,
            stack_info,
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

            // Add to zombie queue for cleanup
            self.zombie_queue.push(tid);

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
        // 1. If current thread is still running, add it back to ready queue
        if let Some(current) = self.current_tid {
            let (state, priority) = if let Some(tcb) = self.get_thread(current) {
                (Some(tcb.state), Some(tcb.priority))
            } else {
                (None, None)
            };

            if state == Some(ThreadState::Running) {
                if let Some(p) = priority {
                    // Mark as runnable and requeue
                    if let Some(t) = self.get_thread_mut(current) {
                        t.state = ThreadState::Runnable;
                    }
                    self.ready_queue.enqueue(current, p);
                }
            }
        }

        // 2. Get highest priority runnable thread
        if let Some(next_tid) = self.ready_queue.dequeue() {
            if let Some(tcb) = self.get_thread_mut(next_tid) {
                tcb.state = ThreadState::Running;
            }
            self.current_tid = Some(next_tid);
            Some(next_tid)
        } else {
            // 3. No runnable threads, run idle thread
            if let Some(idle_tid) = self.idle_tid {
                if let Some(tcb) = self.get_thread_mut(idle_tid) {
                    tcb.state = ThreadState::Running;
                }
            }
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

    /// Reap zombie threads and free their resources
    pub fn reap_zombies(&mut self) {
        if self.zombie_queue.is_empty() {
            return;
        }

        let mut still_zombie = Vec::new();
        let mut to_reap = Vec::new();

        // Separate threads that can be reaped from those still running
        while let Some(tid) = self.zombie_queue.pop() {
            if Some(tid) == self.current_tid {
                // Cannot reap current thread yet
                still_zombie.push(tid);
            } else {
                to_reap.push(tid);
            }
        }
        self.zombie_queue = still_zombie;

        for tid in to_reap {
            if let Some(tcb) = self.threads[tid as usize].take() {
                // 1. Free kernel stack
                let stack_phys = tcb.kernel_stack_phys;
                let stack_pages = tcb.kernel_stack_pages;
                crate::memory::FRAME_ALLOCATOR.deallocate_contiguous(
                    x86_64::structures::paging::PhysFrame::containing_address(stack_phys),
                    stack_pages,
                );

                // 2. TCB (Box<ThreadControlBlock>) will be dropped here as it goes out of scope
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
/// Returns (top_addr, phys_base, page_count)
fn allocate_kernel_stack(size: usize) -> (usize, PhysAddr, usize) {
    use crate::memory::frame::FRAME_ALLOCATOR;
    use crate::memory::paging::phys_to_virt;

    let pages = size.div_ceil(4096);

    let frame = FRAME_ALLOCATOR
        .allocate_contiguous(pages)
        .expect("Failed to allocate kernel stack");

    // Get virtual address via HHDM
    let phys_addr = frame.start_address();
    let virt_addr = phys_to_virt(phys_addr);

    // Stack grows down, so return top address (virtual)
    let top = virt_addr.as_u64() as usize + (pages * 4096);
    (top, phys_addr, pages)
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
}
