//! Thread Control Block for Proka Kernel
//!
//! This module implements the thread management with clear separation
//! from process management. Threads are execution units that belong to
//! a process and share the process's resources.
//!
//! Design principles:
//! - Thread contains only execution-related state (context, kernel stack)
//! - Process-level resources (address space, file descriptors) are in PCB
//! - Thread references its parent process via pid

use crate::process::process::Pid;
use x86_64::PhysAddr;

/// Thread ID type
pub type Tid = u16;

/// Kernel thread ID (reserved for idle thread)
pub const KERNEL_TID: Tid = 0;

/// Maximum number of threads supported
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
///
/// Contains only thread-specific execution state.
/// Process-level resources are accessed through the parent process.
#[derive(Debug)]
pub struct ThreadControlBlock {
    /// Thread ID
    pub tid: Tid,
    /// Process ID (parent process)
    pub pid: Pid,
    /// Current state
    pub state: ThreadState,
    /// Priority (0-255, 0 is highest)
    pub priority: u8,
    /// CPU context for switching
    pub context: Context,
    /// Kernel stack top (Virtual) - thread-specific
    pub kernel_stack_top: usize,
    /// Kernel stack physical base (for deallocation)
    pub kernel_stack_phys: PhysAddr,
    /// Kernel stack size in pages
    pub kernel_stack_pages: usize,
    /// User stack top (if user thread) - cached from process
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
    ///
    /// Kernel threads share the kernel address space and don't have
    /// a separate user stack or vspace.
    pub fn new_kernel(
        tid: Tid,
        pid: Pid,
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
            pid,
            state: ThreadState::Runnable,
            priority,
            context,
            kernel_stack_top,
            kernel_stack_phys,
            kernel_stack_pages,
            user_stack_top: None,
            entry_point: entry_point as usize,
            tls_ptr: None,
            name: None,
        }
    }

    /// Create a new user thread
    ///
    /// User threads belong to a process and will use the process's
    /// address space (vspace). The vspace is managed by the process,
    /// not stored in the thread.
    pub fn new_user(
        tid: Tid,
        pid: Pid,
        priority: u8,
        entry_point: usize,
        user_stack_top: usize,
        stack_info: (usize, PhysAddr, usize),
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
            pid,
            state: ThreadState::Runnable,
            priority,
            context,
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

    /// Check if this is a kernel thread
    pub fn is_kernel_thread(&self) -> bool {
        self.user_stack_top.is_none()
    }

    /// Get the thread's full identifier (pid:tid)
    pub fn full_id(&self) -> alloc::string::String {
        alloc::format!("{}:{}", self.pid, self.tid)
    }
}
