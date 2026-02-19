# 进程管理 (Process Management)

Proka 内核的进程管理子系统负责管理进程的生命周期、资源分配和父子关系。进程是资源（内存、文件描述符）的所有者，而线程是 CPU 调度的基本单位。

## 设计理念

- **进程-线程分离**：进程拥有资源，线程执行代码
- **层级关系**：进程之间存在父子关系，形成进程树
- **资源管理**：进程负责管理其资源（内存空间、文件描述符）
- **生命周期管理**：进程创建、执行、终止、回收的完整生命周期

## 核心数据结构

### ProcessControlBlock (PCB)

进程控制块定义在 `kernel/src/process/process.rs`：

```rust
pub struct ProcessControlBlock {
    pub pid: Pid,                    // 进程 ID
    pub ppid: Pid,                   // 父进程 ID
    pub status: ProcessStatus,       // 进程状态
    pub exit_code: Option<i32>,      // 退出码
    pub memory_set: Arc<Mutex<MemorySet>>,  // 地址空间
    pub fds: Arc<Mutex<Vec<Option<Arc<File>>>>>,  // 文件描述符表
    pub cwd: String,                 // 当前工作目录
    pub threads: Vec<Tid>,           // 线程列表
    pub main_thread_tid: Option<Tid>, // 主线程 ID
    pub children: Vec<Pid>,          // 子进程列表
    pub signal_mask: u64,            // 信号掩码
}
```

### ProcessStatus

进程状态枚举：

- **Ready**: 进程已创建，等待第一个线程运行
- **Running**: 至少有一个线程正在运行
- **Blocked**: 进程被阻塞（如等待子进程）
- **Zombie**: 进程已终止，等待父进程回收

## 进程生命周期

### 1. 进程创建

```rust
// 创建新进程
pub fn create_process(&mut self, ppid: Pid, memory_set: MemorySet) -> Result<Pid, ()>
```

创建流程：
1. 分配新的 PID
2. 创建 PCB 并初始化资源
3. 将新进程添加到父进程的 children 列表
4. 返回新进程的 PID

### 2. 进程执行

进程本身不执行代码，而是通过其线程执行：

```rust
// 创建用户线程（属于某个进程）
pub fn create_user_thread(
    &mut self,
    pid: Pid,              // 所属进程
    entry_point: usize,
    user_stack_top: usize,
    priority: u8,
    name: Option<&str>,
) -> Result<Tid, SchedulerError>
```

### 3. 进程终止

当进程的最后一个线程终止时，进程变为 Zombie 状态：

```rust
// 调度器在线程终止时调用
fn terminate_thread(&mut self, tid: Tid) -> Result<(), SchedulerError> {
    // ... 终止线程 ...
    
    // 通知进程管理器
    if pcb.remove_thread(tid) {
        // 最后一个线程退出，进程变为 Zombie
        pcb.status = ProcessStatus::Zombie;
        pcb.exit_code = Some(0);
    }
}
```

### 4. 进程回收

父进程通过 `waitpid` 系统调用回收子进程：

```rust
pub fn wait_child(&mut self, pid: Pid, target_pid: Option<Pid>) -> Option<(Pid, i32)>
```

## 资源管理

### 文件描述符管理

```rust
// 分配新的文件描述符
pub fn alloc_fd(&mut self) -> Option<Fd>

// 打开文件
pub fn open_file(&mut self, file: Arc<File>) -> Option<Fd>

// 获取文件
pub fn get_file(&self, fd: Fd) -> Option<Arc<File>>

// 关闭文件描述符
pub fn close_fd(&mut self, fd: Fd) -> bool

// 复制文件描述符
pub fn dup_fd(&mut self, old_fd: Fd) -> Option<Fd>

// 复制到指定位置
pub fn dup2_fd(&mut self, old_fd: Fd, new_fd: Fd) -> Option<Fd>
```

### 内存管理

进程的地址空间通过 `MemorySet` 管理：

```rust
pub struct ProcessControlBlock {
    pub memory_set: Arc<Mutex<MemorySet>>,
    // ...
}
```

`MemorySet` 包含：
- 页表（Page Table）
- 虚拟内存区域列表（VMA）
- 堆区域管理

## 进程间关系

### 父子关系

- 每个进程（除了 init）都有一个父进程
- 父进程可以通过 `waitpid` 等待子进程终止
- 子进程终止时，父进程会收到通知（通过 zombie queue）

### 进程树

```
PID 0 (kernel)
  ├── PID 1 (init)
  │     ├── PID 2 (shell)
  │     │     ├── PID 3 (ls)
  │     │     └── PID 4 (cat)
  │     └── PID 5 (daemon)
  └── PID 6 (kthread)
```

## 系统调用

### 进程相关

| 系统调用 | 功能 | 参数 |
|---------|------|------|
| `sys_exit` | 退出当前进程 | `exit_code: i32` |
| `sys_getpid` | 获取当前进程 ID | - |
| `sys_getppid` | 获取父进程 ID | - |
| `sys_waitpid` | 等待子进程终止 | `pid: Pid, exit_code_ptr: *mut i32` |
| `sys_fork` | 创建子进程（未实现） | - |
| `sys_execve` | 执行新程序（未实现） | `path, argv, envp` |

### 线程相关

| 系统调用 | 功能 | 参数 |
|---------|------|------|
| `sys_gettid` | 获取当前线程 ID | - |
| `sys_create_thread` | 创建新线程 | `entry_point, stack_top, priority` |
| `sys_exit_thread` | 退出当前线程 | - |
| `sys_yield` | 让出 CPU | - |

### 文件描述符相关

| 系统调用 | 功能 | 参数 |
|---------|------|------|
| `sys_open` | 打开文件（未实现） | `path, flags` |
| `sys_close` | 关闭文件描述符 | `fd: Fd` |
| `sys_dup` | 复制文件描述符 | `old_fd: Fd` |
| `sys_dup2` | 复制到指定位置 | `old_fd, new_fd: Fd` |

### 目录相关

| 系统调用 | 功能 | 参数 |
|---------|------|------|
| `sys_getcwd` | 获取当前工作目录 | `buf, size` |
| `sys_chdir` | 改变当前工作目录 | `path` |

## 实现细节

### 内核进程 (PID 0)

- 特殊的内核进程，所有内核线程都属于它
- 拥有内核地址空间
- 在系统启动时创建

### Zombie 进程处理

当进程终止时：
1. 进程状态变为 Zombie
2. 退出码被保存
3. 进程被添加到 zombie queue
4. 父进程通过 `waitpid` 回收

如果父进程先终止：
- 子进程被重新分配给 init 进程（PID 1）
- init 进程负责回收这些孤儿进程

### 地址空间切换

当调度器切换线程时，如果新旧线程属于不同进程，需要切换地址空间：

```rust
if old_pid != new_pid {
    // Switch address space
    let new_pcb = process::lock().get_process(new_pid).unwrap();
    let pml4_addr = new_pcb.memory_set.lock().page_table.level_4_table().addr();
    unsafe {
        x86_64::registers::control::Cr3::write(
            PhysFrame::from_start_address_unchecked(pml4_addr),
            Cr3Flags::empty(),
        );
    }
}
```

## 待完善内容

- [ ] `fork` 系统调用实现
- [ ] `execve` 系统调用实现
- [ ] 信号机制
- [ ] 进程组与会话
- [ ] 资源限制 (rlimit)
- [ ] 进程间通信 (IPC) 机制
