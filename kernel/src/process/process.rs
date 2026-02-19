//! Process management for Proka Kernel

use crate::fs::vfs::File;
use crate::memory::paging::vmm::MemorySet;
use crate::process::thread::Tid;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, Once};
use x86_64::PhysAddr;

/// Process ID type
pub type Pid = u16;

/// Kernel Process ID (reserved)
pub const KERNEL_PID: Pid = 0;

/// File descriptor type
pub type Fd = usize;

/// Standard file descriptors
pub const STDIN_FILENO: Fd = 0;
pub const STDOUT_FILENO: Fd = 1;
pub const STDERR_FILENO: Fd = 2;

/// Maximum number of file descriptors per process
pub const MAX_FD_COUNT: usize = 256;

/// Process status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    /// Process is created but no threads are running yet
    Ready,
    /// One or more threads are running
    Running,
    /// Process is blocked (e.g., waiting for child)
    Blocked,
    /// Process has terminated, waiting for parent to reap
    Zombie,
}

/// Process Control Block (PCB)
pub struct ProcessControlBlock {
    pub pid: Pid,
    pub ppid: Pid,
    pub status: ProcessStatus,
    pub exit_code: Option<i32>,
    pub vspace: PhysAddr,
    pub memory_set: Arc<Mutex<MemorySet>>,
    pub fds: Arc<Mutex<Vec<Option<Arc<File>>>>>,
    pub cwd: String,
    pub threads: Vec<Tid>,
    pub main_thread_tid: Option<Tid>,
    /// Child processes
    pub children: Vec<Pid>,
    /// Signal mask (simplified)
    pub signal_mask: u64,
}

impl ProcessControlBlock {
    pub fn new(pid: Pid, ppid: Pid, memory_set: MemorySet) -> Self {
        use x86_64::VirtAddr;
        let vspace = {
            let pt_virt = VirtAddr::from_ptr(memory_set.page_table.level_4_table() as *const _);
            // In our kernel, HHDM offset is used.
            // We can get the physical address by subtracting the HHDM offset.
            // virt_to_phys_direct requires the address to be in HHDM region.
            unsafe { crate::memory::paging::virt_to_phys_direct(pt_virt) }
        };

        Self {
            pid,
            ppid,
            status: ProcessStatus::Ready,
            exit_code: None,
            vspace,
            memory_set: Arc::new(Mutex::new(memory_set)),
            fds: Arc::new(Mutex::new(alloc::vec![
                None, // stdin
                None, // stdout
                None, // stderr
            ])),
            cwd: String::from("/"),
            threads: Vec::new(),
            main_thread_tid: None,
            children: Vec::new(),
            signal_mask: 0,
        }
    }

    /// Add a thread to this process
    pub fn add_thread(&mut self, tid: Tid) {
        // First thread added is the main thread
        if self.main_thread_tid.is_none() {
            self.main_thread_tid = Some(tid);
        }
        self.threads.push(tid);
        self.status = ProcessStatus::Running;
    }

    /// Remove a thread from this process. Returns true if it was the last thread.
    pub fn remove_thread(&mut self, tid: Tid) -> bool {
        self.threads.retain(|&t| t != tid);
        if self.main_thread_tid == Some(tid) {
            // If main thread exits, mark it as None.
            self.main_thread_tid = None;
        }
        self.threads.is_empty()
    }

    /// Check if process is ready to be reaped
    pub fn is_zombie(&self) -> bool {
        matches!(self.status, ProcessStatus::Zombie)
    }

    /// Allocate a new file descriptor
    pub fn alloc_fd(&mut self) -> Option<Fd> {
        let mut fds = self.fds.lock();

        // First, try to find a free slot
        for (fd, file_opt) in fds.iter().enumerate() {
            if file_opt.is_none() {
                return Some(fd);
            }
        }

        // If no free slot, extend the vector
        if fds.len() < MAX_FD_COUNT {
            let fd = fds.len();
            fds.push(None);
            return Some(fd);
        }

        None
    }

    /// Open a file and return its file descriptor
    pub fn open_file(&mut self, file: Arc<File>) -> Option<Fd> {
        let fd = self.alloc_fd()?;
        let mut fds = self.fds.lock();
        fds[fd] = Some(file);
        Some(fd)
    }

    /// Get a file by its file descriptor
    pub fn get_file(&self, fd: Fd) -> Option<Arc<File>> {
        let fds = self.fds.lock();
        fds.get(fd).and_then(|f| f.clone())
    }

    /// Close a file descriptor
    pub fn close_fd(&mut self, fd: Fd) -> bool {
        let mut fds = self.fds.lock();
        if fd < fds.len() && fds[fd].is_some() {
            fds[fd] = None;
            true
        } else {
            false
        }
    }

    /// Duplicate a file descriptor
    pub fn dup_fd(&mut self, old_fd: Fd) -> Option<Fd> {
        let file = self.get_file(old_fd)?;
        self.open_file(file)
    }

    /// Duplicate a file descriptor to a specific new fd
    pub fn dup2_fd(&mut self, old_fd: Fd, new_fd: Fd) -> Option<Fd> {
        let file = self.get_file(old_fd)?;

        // Ensure the fds vector is large enough
        let mut fds = self.fds.lock();
        while fds.len() <= new_fd {
            if fds.len() >= MAX_FD_COUNT {
                return None;
            }
            fds.push(None);
        }

        fds[new_fd] = Some(file);
        Some(new_fd)
    }

    /// Add a child process
    pub fn add_child(&mut self, child_pid: Pid) {
        self.children.push(child_pid);
    }

    /// Remove a child process
    pub fn remove_child(&mut self, child_pid: Pid) {
        self.children.retain(|&pid| pid != child_pid);
    }

    /// Check if this process has any children
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Get the list of children
    pub fn get_children(&self) -> &[Pid] {
        &self.children
    }

    /// Set current working directory
    pub fn set_cwd(&mut self, path: &str) {
        self.cwd = String::from(path);
    }

    /// Get current working directory
    pub fn get_cwd(&self) -> &str {
        &self.cwd
    }
}

/// Process Manager
pub struct ProcessManager {
    processes: Vec<Option<Arc<Mutex<ProcessControlBlock>>>>,
    next_pid: Pid,
    /// Zombie processes waiting to be reaped (pid -> (ppid, exit_code))
    zombie_queue: Vec<(Pid, Pid, i32)>,
}

impl ProcessManager {
    pub const fn new() -> Self {
        Self {
            processes: Vec::new(),
            next_pid: KERNEL_PID + 1,
            zombie_queue: Vec::new(),
        }
    }

    /// Create the initial kernel process (pid 0)
    pub fn create_kernel_process(&mut self, memory_set: MemorySet) {
        let pcb = Arc::new(Mutex::new(ProcessControlBlock::new(
            KERNEL_PID, KERNEL_PID, memory_set,
        )));
        self.processes.push(Some(pcb));
    }

    /// Create a new user process
    pub fn create_process(&mut self, ppid: Pid, memory_set: MemorySet) -> Result<Pid, ()> {
        let pid = self.alloc_pid().ok_or(())?;
        let pcb = Arc::new(Mutex::new(ProcessControlBlock::new(pid, ppid, memory_set)));

        // Add child to parent
        if let Some(parent_pcb) = self.get_process(ppid) {
            parent_pcb.lock().add_child(pid);
        }

        while self.processes.len() <= pid as usize {
            self.processes.push(None);
        }
        self.processes[pid as usize] = Some(pcb);
        Ok(pid)
    }

    /// Get a process by its PID
    pub fn get_process(&self, pid: Pid) -> Option<Arc<Mutex<ProcessControlBlock>>> {
        self.processes.get(pid as usize).and_then(|p| p.clone())
    }

    /// Terminate a process and mark it as Zombie
    pub fn terminate_process(&mut self, pid: Pid, exit_code: i32) -> Option<Vec<Tid>> {
        if let Some(pcb_arc) = self.get_process(pid) {
            let mut pcb = pcb_arc.lock();
            pcb.status = ProcessStatus::Zombie;
            pcb.exit_code = Some(exit_code);
            let ppid = pcb.ppid;
            // Return list of threads to be terminated by scheduler
            let threads = pcb.threads.clone();
            drop(pcb);

            // Add to zombie queue for parent to reap
            self.zombie_queue.push((pid, ppid, exit_code));

            Some(threads)
        } else {
            None
        }
    }

    /// Wait for a child process to terminate
    /// Returns (pid, exit_code) of the terminated child, or None if no children
    pub fn wait_child(&mut self, pid: Pid, target_pid: Option<Pid>) -> Option<(Pid, i32)> {
        // Check if we have any children
        let pcb = self.get_process(pid)?;
        if !pcb.lock().has_children() {
            return None;
        }

        // Look for a zombie child
        let pos = if let Some(target) = target_pid {
            // waitpid - wait for specific child
            self.zombie_queue
                .iter()
                .position(|(zombie_pid, ppid, _)| *ppid == pid && *zombie_pid == target)
        } else {
            // wait - wait for any child
            self.zombie_queue
                .iter()
                .position(|(_, ppid, _)| *ppid == pid)
        };

        if let Some(pos) = pos {
            let (child_pid, _, exit_code) = self.zombie_queue.remove(pos);

            // Remove child from parent's children list
            pcb.lock().remove_child(child_pid);

            // Clean up the process
            self.processes[child_pid as usize] = None;

            Some((child_pid, exit_code))
        } else {
            None
        }
    }

    /// Check if a process has zombie children
    pub fn has_zombie_child(&self, pid: Pid) -> bool {
        self.zombie_queue.iter().any(|(_, ppid, _)| *ppid == pid)
    }

    /// Get current process (for system calls)
    pub fn current_process(&self) -> Option<Arc<Mutex<ProcessControlBlock>>> {
        // Get current thread's pid
        let current_tid = crate::process::scheduler::current_tid()?;
        let scheduler = crate::process::scheduler::SCHEDULER.lock();
        let tcb = scheduler.as_ref()?.get_thread(current_tid)?;
        let pid = tcb.pid;
        drop(scheduler);
        self.get_process(pid)
    }

    /// Allocate a new PID
    fn alloc_pid(&mut self) -> Option<Pid> {
        let start_pid = self.next_pid;
        let max_pid = u16::MAX;

        for pid in start_pid..=max_pid {
            let idx = pid as usize;
            if idx >= self.processes.len() || self.processes[idx].is_none() {
                self.next_pid = pid.saturating_add(1);
                return Some(pid);
            }
        }
        for pid in (KERNEL_PID + 1)..start_pid {
            let idx = pid as usize;
            if idx >= self.processes.len() || self.processes[idx].is_none() {
                self.next_pid = pid.saturating_add(1);
                return Some(pid);
            }
        }
        None
    }
}

static PROCESS_MANAGER: Once<Mutex<ProcessManager>> = Once::new();

/// Initialize the process manager system
pub fn init() {
    let memory_set = crate::memory::paging::vmm::KERNEL_MEMORY_SET
        .lock()
        .take()
        .expect("Kernel memory set not initialized");

    PROCESS_MANAGER.call_once(|| {
        let mut manager = ProcessManager::new();
        // The kernel process takes ownership of the kernel's memory set
        manager.create_kernel_process(memory_set);
        Mutex::new(manager)
    });
    log::info!("Process manager initialized");
}

/// Get a lock to the global process manager
pub fn lock() -> spin::MutexGuard<'static, ProcessManager> {
    PROCESS_MANAGER
        .get()
        .expect("Process manager not initialized")
        .lock()
}
