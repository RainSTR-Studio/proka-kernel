# 调度器 (Scheduler)

Proka 内核的调度器负责管理和切换线程，是实现并发和多任务处理的核心组件。我们采用了一种高度模块化和可插拔的设计，允许在运行时或编译时轻松替换调度算法。

## 设计理念

调度器的核心设计目标是 **解耦** 和 **灵活性**。

- **抽象与实现分离**：通过 `Scheduler` Trait 定义调度器的标准行为，将接口与具体的调度算法（如优先级调度、时间片轮转等）完全分开。
- **可插拔性**：内核持有一个全局的 `Scheduler` trait object (`Box<dyn Scheduler>`)。这意味着可以动态替换调度器实现，为测试、性能分析或特定应用场景下的优化提供了极大的便利。
- **进程与线程分离**：明确了进程 (Process) 作为资源（内存、文件描述符）的所有者，而线程 (Thread) 作为被调度的执行单元。每个线程都必须属于一个进程。

## 核心抽象

调度器模块围绕以下几个核心抽象构建：

### `Scheduler` Trait

所有调度算法都必须实现 `Scheduler` trait (`kernel/src/process/scheduler.rs`)，它定义了调度器的标准接口：

- `init()`: 初始化调度器，创建空闲线程 (idle thread)。
- `schedule()`: 核心调度函数，决定下一个要运行的线程。
- `create_kernel_thread()` / `create_user_thread()`: 创建新的内核或用户线程。
- `terminate_thread()`: 终止一个线程，将其标记为待回收状态。
- `block_ipc()` / `unblock()`: 改变线程状态以支持阻塞操作（如 IPC）。
- `get_thread()` / `get_thread_mut()`: 获取线程控制块 (TCB) 的引用。

### `SchedulerError`

一个统一的错误枚举，覆盖了所有调度操作可能遇到的问题，例如 `MaxThreadsReached` 或 `ThreadNotFound`。

## 进程与线程模型

- **进程 (`ProcessControlBlock`, PCB)**: 定义在 `kernel/src/process/process.rs`。它是资源管理的单位，拥有：
    - 独立的虚拟内存空间 (`MemorySet`)。
    - 文件描述符表 (`fds`)。
    - 当前工作目录 (`cwd`)。
    - 一个或多个线程。

- **线程 (`ThreadControlBlock`, TCB)**: 定义在 `kernel/src/process/thread.rs`。它是CPU调度的基本单位，包含：
    - 唯一的线程ID (`Tid`) 和所属进程的ID (`Pid`)。
    - 线程状态（如 `Running`, `Runnable`, `BlockedIpc`）。
    - 寄存器上下文 (`Context`)，用于在切换时保存和恢复状态。
    - 内核栈。

## 具体实现

Proka 内核目前内置了两种调度器实现，位于 `kernel/src/process/schedulers/` 目录下。

### 1. 优先级调度器 (`PriorityScheduler`)

这是内核默认的调度器，实现了基于优先级的抢占式调度。

- **工作原理**: 调度器维护一个由 256 个队列组成的 `PriorityQueue`，每个队列对应一个优先级（0-255，0为最高）。调度时，总会从最高优先级的非空队列中选择下一个线程来执行。
- **公平性**: 为了防止低优先级线程"饿死" (starvation)，`PriorityQueue` 的出队逻辑包含一个简单的公平性机制：每隔一定次数的调度，会尝试从较低优先级的队列中选择线程。
- **适用场景**: 适用于需要区分任务重要性的场景，确保高优先级任务（如驱动程序、中断处理）能够及时响应。

### 2. 时间片轮转调度器 (`RoundRobinScheduler`)

这是一个更简单的、非优先级的调度器实现。

- **工作原理**: 所有可运行的线程被放在一个先进先出 (FIFO) 队列中。每次时钟中断 (`timer_tick`) 触发调度时，当前线程被移到队尾，队列头部的线程成为下一个运行的线程。
- **适用场景**: 作为可插拔调度器架构的简单示例，适用于所有任务优先级相同的场景。

## 集成与使用

- **初始化**: 内核在 `kernel_main` 函数中通过调用 `scheduler::init()` 来初始化默认的 `PriorityScheduler`。可以通过 `scheduler::set_scheduler()` 函数来替换为其他调度器。
- **时钟中断 (`timer_tick`)**: APIC Timer 的中断处理程序会调用 `scheduler::timer_tick()`，这会触发一次抢占式调度，确保没有线程可以无限期地占用CPU。
- **主动让出 (`yield_thread`)**: 线程可以主动调用 `scheduler::yield_thread()` 来放弃CPU，触发一次调度，让其他线程运行。
- **阻塞与唤醒**: 当线程需要等待资源（例如，等待一个 IPC 消息）时，调度器会将其状态设置为 `Blocked` 并从就绪队列中移除。当资源可用时，其他代码（如 IPC 子系统）会调用 `scheduler::unblock()` 来唤醒该线程，使其重新进入_就绪队列。
