pub mod context_switch;
pub mod process;
pub mod scheduler;
pub mod schedulers;
pub mod thread;

use crate::process::process::{Pid, ProcessControlBlock};
use crate::process::scheduler::SchedulerError;
use crate::process::thread::Tid;
use crate::sync::mutex::Mutex;
use alloc::sync::Arc;

/// 快速启动一个内核线程 (Spawn a kernel thread)
///
/// 这是一个高层封装，会自动处理调度器锁定。
/// 默认优先级为 128 (中等)
pub fn spawn_kthread(
    entry: extern "C" fn() -> !,
    priority: u8,
    name: &str,
) -> Result<Tid, SchedulerError> {
    scheduler::create_kernel_thread(entry, priority, name)
}

/// 启动一个高优先级内核线程
pub fn spawn_kthread_high(entry: extern "C" fn() -> !, name: &str) -> Result<Tid, SchedulerError> {
    spawn_kthread(entry, 64, name)
}

/// 启动一个低优先级内核线程
pub fn spawn_kthread_low(entry: extern "C" fn() -> !, name: &str) -> Result<Tid, SchedulerError> {
    spawn_kthread(entry, 192, name)
}

/// 启动一个高优先级服务
pub fn spawn_service_high(name: &str, entry: extern "C" fn() -> !) -> Result<Tid, SchedulerError> {
    spawn_service(name, entry, 64)
}

/// 创建用户进程 (预留接口)
///
/// 当前为占位实现，返回错误。
/// 后续需要实现 ELF 加载器后才能正常工作。
pub fn create_user_process(_name: &str, _priority: u8) -> Result<Pid, ()> {
    // TODO: Implement ELF loading and user process creation
    log::warn!("create_user_process is not yet implemented");
    Err(())
}

/// 获取当前运行线程的 ID
pub fn current_tid() -> Option<Tid> {
    scheduler::current_tid()
}

/// 获取当前运行进程的控制块 (PCB)
pub fn current_process() -> Option<Arc<Mutex<ProcessControlBlock>>> {
    process::lock().current_process()
}

/// 获取当前进程的 PID
pub fn current_pid() -> Option<Pid> {
    current_process().map(|p| p.lock().pid)
}

/// 启动一个“服务”线程
///
/// 在微内核架构中，服务通常是长期运行的线程/进程。
/// 此函数为后续自动注册 IPC 服务预留了入口。
pub fn spawn_service(
    name: &str,
    entry: extern "C" fn() -> !,
    priority: u8,
) -> Result<Tid, SchedulerError> {
    let tid = spawn_kthread(entry, priority, name)?;
    log::info!("[Process] Service '{}' spawned (TID: {})", name, tid);
    Ok(tid)
}
