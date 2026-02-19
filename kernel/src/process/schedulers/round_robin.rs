use crate::process::process::{self, Pid};
use crate::process::scheduler::{allocate_kernel_stack, Scheduler, SchedulerError};
use crate::process::thread::{ThreadControlBlock, ThreadState, Tid, KERNEL_TID};
use alloc::vec::Vec;

/// Simple Round-Robin scheduler
pub struct RoundRobinScheduler {
    threads: Vec<Option<alloc::boxed::Box<ThreadControlBlock>>>,
    next_tid: Tid,
    ready_queue: Vec<Tid>,
    current_tid: Option<Tid>,
    idle_tid: Option<Tid>,
    zombie_queue: Vec<Tid>,
}

impl RoundRobinScheduler {
    pub const fn new() -> Self {
        Self {
            threads: Vec::new(),
            next_tid: 1,
            ready_queue: Vec::new(),
            current_tid: None,
            idle_tid: None,
            zombie_queue: Vec::new(),
        }
    }

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

impl Scheduler for RoundRobinScheduler {
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

    fn schedule(&mut self) -> Option<Tid> {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                if tcb.state == ThreadState::Running {
                    tcb.state = ThreadState::Runnable;
                    self.ready_queue.push(current);
                }
            }
        }
        if !self.ready_queue.is_empty() {
            let next_tid = self.ready_queue.remove(0);
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
        self.ready_queue.push(tid);
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
        self.ready_queue.push(tid);
        Ok(tid)
    }

    fn terminate_thread(&mut self, tid: Tid) -> Result<(), SchedulerError> {
        let is_current = self.current_tid == Some(tid);
        let _pid = {
            let tcb = self
                .threads
                .get_mut(tid as usize)
                .and_then(|t| t.as_deref_mut())
                .ok_or(SchedulerError::ThreadNotFound)?;
            tcb.state = ThreadState::Terminated;
            tcb.pid
        };

        if !is_current {
            let pos = self.ready_queue.iter().position(|&t| t == tid);
            if let Some(p) = pos {
                self.ready_queue.remove(p);
            }
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

    fn unblock(&mut self, tid: Tid) -> Result<(), SchedulerError> {
        if let Some(tcb) = self.get_thread_mut(tid) {
            if tcb.is_blocked() {
                tcb.state = ThreadState::Runnable;
                self.ready_queue.push(tid);
                Ok(())
            } else {
                Err(SchedulerError::NotBlocked)
            }
        } else {
            Err(SchedulerError::ThreadNotFound)
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

    fn set_priority(&mut self, tid: Tid, new_priority: u8) -> Result<(), SchedulerError> {
        if let Some(tcb) = self.get_thread_mut(tid) {
            tcb.priority = new_priority;
            Ok(())
        } else {
            Err(SchedulerError::ThreadNotFound)
        }
    }

    fn yield_current(&mut self) {
        if let Some(current) = self.current_tid {
            if let Some(tcb) = self.get_thread_mut(current) {
                if tcb.state == ThreadState::Running {
                    tcb.state = ThreadState::Runnable;
                    self.ready_queue.push(current);
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

            if let Some(Some(tcb)) = self.threads.get(tid as usize) {
                if let ThreadState::Sleeping(until) = tcb.state {
                    if current_uptime_ms >= until {
                        should_wake = true;
                    }
                }
            }

            if should_wake {
                if let Some(tcb) = self.get_thread_mut(tid) {
                    tcb.state = ThreadState::Runnable;
                    self.ready_queue.push(tid);
                }
            }
        }
    }
}
