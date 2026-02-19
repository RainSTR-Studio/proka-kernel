use crate::process::process::{self, Pid};
use crate::process::scheduler::{allocate_kernel_stack, Scheduler, SchedulerError};
use crate::process::thread::{ThreadControlBlock, ThreadState, Tid, KERNEL_TID, NUM_PRIORITIES};
use alloc::vec::Vec;

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

/// Priority-based thread scheduler
pub struct PriorityScheduler {
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

impl PriorityScheduler {
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

    /// Allocate a new TID
    fn alloc_tid(&mut self) -> Result<Tid, SchedulerError> {
        let start_tid = if self.next_tid == 0 { 1 } else { self.next_tid };
        let max_tid = u16::MAX;

        for tid in start_tid..=max_tid {
            let idx = tid as usize;
            if idx >= self.threads.len() || self.threads[idx].is_none() {
                self.next_tid = tid.saturating_add(1);
                return Ok(tid);
            }
        }
        for tid in 1..start_tid {
            let idx = tid as usize;
            if idx >= self.threads.len() || self.threads[idx].is_none() {
                return Ok(tid);
            }
        }
        Err(SchedulerError::MaxThreadsReached)
    }
}

impl Scheduler for PriorityScheduler {
    fn init(&mut self, idle_entry: extern "C" fn() -> !) {
        let stack_info = allocate_kernel_stack(4096);
        // Idle thread belongs to kernel process (pid 0)
        let mut idle_tcb = ThreadControlBlock::new_kernel(KERNEL_TID, 0, 0, idle_entry, stack_info);
        idle_tcb.set_name("idle");
        idle_tcb.state = ThreadState::Running;

        while self.threads.len() <= KERNEL_TID as usize {
            self.threads.push(None);
        }
        self.threads[KERNEL_TID as usize] = Some(alloc::boxed::Box::new(idle_tcb));
        self.idle_tid = Some(KERNEL_TID);
        self.current_tid = Some(KERNEL_TID);
    }

    fn set_priority(&mut self, tid: Tid, new_priority: u8) -> Result<(), SchedulerError> {
        if let Some(tcb) = self.get_thread_mut(tid) {
            let old_priority = tcb.priority;
            if old_priority == new_priority {
                return Ok(());
            }
            tcb.priority = new_priority;
            if tcb.state == ThreadState::Runnable {
                self.ready_queue.remove(tid, old_priority);
                self.ready_queue.enqueue(tid, new_priority);
            }
            Ok(())
        } else {
            Err(SchedulerError::ThreadNotFound)
        }
    }

    fn create_kernel_thread(
        &mut self,
        entry_point: extern "C" fn() -> !,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError> {
        let tid = self.alloc_tid()?;
        let stack_info = allocate_kernel_stack(8192);
        // Kernel threads belong to kernel process (pid 0)
        let mut tcb = ThreadControlBlock::new_kernel(tid, 0, priority, entry_point, stack_info);
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

    fn create_user_thread(
        &mut self,
        pid: Pid,
        entry_point: usize,
        user_stack_top: usize,
        priority: u8,
        name: Option<&str>,
    ) -> Result<Tid, SchedulerError> {
        // Ensure the process exists
        let pcb_arc = process::lock()
            .get_process(pid)
            .ok_or(SchedulerError::PidNotFound)?;

        let tid = self.alloc_tid()?;
        let kernel_stack_size = 8192; // Default kernel stack for user threads
        let stack_info = allocate_kernel_stack(kernel_stack_size);

        let mut tcb = ThreadControlBlock::new_user(
            tid,
            pid,
            priority,
            entry_point,
            user_stack_top,
            stack_info,
        );
        if let Some(n) = name {
            tcb.set_name(n);
        }

        // Add thread to process
        pcb_arc.lock().add_thread(tid);

        while self.threads.len() <= tid as usize {
            self.threads.push(None);
        }
        self.threads[tid as usize] = Some(alloc::boxed::Box::new(tcb));
        self.ready_queue.enqueue(tid, priority);
        Ok(tid)
    }

    fn terminate_thread(&mut self, tid: Tid) -> Result<(), SchedulerError> {
        let is_current = self.current_tid == Some(tid);
        let (pid, priority) = {
            let tcb = self
                .get_thread_mut(tid)
                .ok_or(SchedulerError::ThreadNotFound)?;
            tcb.state = ThreadState::Terminated;
            (tcb.pid, tcb.priority)
        };

        if !is_current {
            self.ready_queue.remove(tid, priority);
        }

        self.zombie_queue.push(tid);

        // Notify joined threads
        for other_tid in 0..self.threads.len() {
            let other_tid = other_tid as Tid;
            let mut should_wake = false;
            if let Some(Some(tcb)) = self.threads.get(other_tid as usize) {
                if let ThreadState::BlockedJoin(target) = tcb.state {
                    if target == tid {
                        should_wake = true;
                    }
                }
            }
            if should_wake {
                let _ = self.unblock(other_tid);
            }
        }

        // Notify process manager
        if let Some(pcb_arc) = process::lock().get_process(pid) {
            let mut pcb = pcb_arc.lock();
            if pcb.remove_thread(tid) {
                // Last thread of the process has exited
                log::info!("Process {} became a zombie", pid);
                pcb.status = process::ProcessStatus::Zombie;
                pcb.exit_code = Some(0); // Default exit code
            }
        }

        Ok(())
    }

    fn block_ipc(&mut self, sender_tid: Option<Tid>, timeout_ms: Option<u64>) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                tcb.state = ThreadState::BlockedIpc {
                    sender_tid,
                    timeout_ms,
                };
            }
        }
    }

    fn block_sleep(&mut self, until_ms: u64) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                tcb.state = ThreadState::Sleeping(until_ms);
            }
        }
    }

    fn block_wait(&mut self, target_pid: Option<Pid>) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                tcb.state = ThreadState::BlockedWait(target_pid);
            }
        }
    }

    fn block_join(&mut self, target_tid: Tid) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                tcb.state = ThreadState::BlockedJoin(target_tid);
            }
        }
    }

    fn block_sync(&mut self, sync_id: u64) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                tcb.state = ThreadState::BlockedSync(sync_id);
            }
        }
    }

    fn unblock_sync(&mut self, sync_id: u64) {
        let mut to_wake = Vec::new();
        for tid in 0..self.threads.len() {
            let tid = tid as Tid;
            if let Some(Some(tcb)) = self.threads.get(tid as usize) {
                if let ThreadState::BlockedSync(id) = tcb.state {
                    if id == sync_id {
                        to_wake.push(tid);
                    }
                }
            }
        }

        for tid in to_wake {
            let _ = self.unblock(tid);
        }
    }

    fn unblock(&mut self, tid: Tid) -> Result<(), SchedulerError> {
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

    fn schedule(&mut self) -> Option<Tid> {
        if let Some(current) = self.current_tid {
            let (state, priority) = if let Some(tcb) = self.get_thread(current) {
                (Some(tcb.state), Some(tcb.priority))
            } else {
                (None, None)
            };

            if state == Some(ThreadState::Running) {
                if let Some(p) = priority {
                    if let Some(t) = self.get_thread_mut(current) {
                        t.state = ThreadState::Runnable;
                    }
                    self.ready_queue.enqueue(current, p);
                }
            }
        }

        if let Some(next_tid) = self.ready_queue.dequeue() {
            if let Some(tcb) = self.get_thread_mut(next_tid) {
                tcb.state = ThreadState::Running;
            }
            self.current_tid = Some(next_tid);
            Some(next_tid)
        } else {
            if let Some(idle_tid) = self.idle_tid {
                if let Some(tcb) = self.get_thread_mut(idle_tid) {
                    tcb.state = ThreadState::Running;
                }
            }
            self.current_tid = self.idle_tid;
            self.idle_tid
        }
    }

    fn current_tid(&self) -> Option<Tid> {
        self.current_tid
    }

    fn get_thread(&self, tid: Tid) -> Option<&ThreadControlBlock> {
        self.threads.get(tid as usize).and_then(|t| t.as_deref())
    }

    fn get_thread_mut(&mut self, tid: Tid) -> Option<&mut ThreadControlBlock> {
        self.threads
            .get_mut(tid as usize)
            .and_then(|t| t.as_deref_mut())
    }

    fn yield_current(&mut self) {
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

    fn reap_zombies(&mut self) {
        if self.zombie_queue.is_empty() {
            return;
        }
        let mut still_zombie = Vec::new();
        let mut to_reap = Vec::new();
        while let Some(tid) = self.zombie_queue.pop() {
            if Some(tid) == self.current_tid {
                still_zombie.push(tid);
            } else {
                to_reap.push(tid);
            }
        }
        self.zombie_queue = still_zombie;
        for tid in to_reap {
            if let Some(tcb) = self.threads[tid as usize].take() {
                let stack_phys = tcb.kernel_stack_phys;
                let stack_pages = tcb.kernel_stack_pages;
                crate::memory::FRAME_ALLOCATOR.deallocate_contiguous(
                    x86_64::structures::paging::PhysFrame::containing_address(stack_phys),
                    stack_pages,
                );
            }
        }
    }

    fn wake_sleeping_threads(&mut self, current_uptime_ms: u64) {
        for tid in 0..self.threads.len() {
            let tid = tid as Tid;
            let mut should_wake = false;
            let mut priority = 0;

            if let Some(Some(tcb)) = self.threads.get(tid as usize) {
                if let ThreadState::Sleeping(until) = tcb.state {
                    if current_uptime_ms >= until {
                        should_wake = true;
                        priority = tcb.priority;
                    }
                }
            }

            if should_wake {
                if let Some(tcb) = self.get_thread_mut(tid) {
                    tcb.state = ThreadState::Runnable;
                    self.ready_queue.enqueue(tid, priority);
                }
            }
        }
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
