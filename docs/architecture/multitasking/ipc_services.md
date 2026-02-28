# IPC 服务架构

Proka Kernel 采用微内核架构，所有系统操作通过 IPC（进程间通信）消息传递完成。

## 架构概览

```text
┌─────────────────────────────────────────────────────────────┐
│                        用户空间                              │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │ 用户程序 │  │ 用户程序 │  │ 文件服务 │  │   设备服务      │ │
│  │         │  │         │  │ (fs:/)  │  │    (dev:/)      │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └───────┬─────────┘ │
└───────┼────────────┼────────────┼───────────────┼───────────┘
        │ syscall    │            │ IPC 消息      │ IPC 消息
        └─────┬──────┴────────────┴───────┬───────┘
              │                           │
              v                           v
┌─────────────────────────────────────────────────────────────┐
│                         内核空间                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              IPC 调度器 (service::dispatch)           │   │
│  └──────────────────────────────────────────────────────┘   │
│                           │                                 │
│         ┌─────────────────┼─────────────────┐              │
│         v                 v                 v              │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐        │
│  │ proc:/     │    │ mem:/      │    │ console:/  │        │
│  │ 进程服务   │    │ 内存服务   │    │ 控制台服务 │        │
│  │ (内核态)   │    │ (内核态)   │    │ (内核态)   │        │
│  └────────────┘    └────────────┘    └────────────┘        │
│                                                             │
│  注: fs:/ 和 dev:/ 可迁移到用户态成为用户空间服务            │
└─────────────────────────────────────────────────────────────┘
```

## 微内核设计原则

### 内核态 vs 用户态服务

| 服务 | 名称 | 当前位置 | 可迁移至用户态 |
|------|------|----------|---------------|
| ProcessService | `proc:/` | 内核态 | ⚠️ 部分功能（需要调度器支持） |
| MemoryService | `mem:/` | 内核态 | ⚠️ 部分功能（需要页表操作） |
| ConsoleService | `console:/` | 内核态 | ✅ 可迁移 |
| FilesystemService | `fs:/` | 未实现 | ✅ 用户态设计 |
| DeviceService | `dev:/` | 未实现 | ✅ 用户态设计 |

### 为什么某些服务保留在内核态？

1. **MemoryService**: 需要直接操作页表，修改地址空间映射
2. **ProcessService**: 需要操作调度器数据结构

### 迁移路径

```text
阶段 1 (当前): 所有服务在内核态
    ↓
阶段 2: Console/FS/Device 迁移到用户态
    ↓
阶段 3: 仅保留最小内核 (IPC + 调度 + 基础内存)
```

## 命名服务注册

### 服务名称格式

服务通过 URI 风格的名称注册：

```
<type>://
```

| 服务名 | ServiceId | 说明 |
|--------|-----------|------|
| `proc:/` | 0 | 进程管理服务 |
| `mem:/` | 1 | 内存管理服务 |
| `console:/` | 2 | 控制台服务 |
| `fs:/` | 3 | 文件系统服务 |
| `dev:/` | 4 | 设备管理服务 |

### 注册内核态服务

内核服务在 `service::init()` 中自动注册：

```rust
// kernel/src/service/mod.rs
pub fn init() {
    register_kernel_service_locked(&mut registry, Box::new(ProcessService::new()));
    register_kernel_service_locked(&mut registry, Box::new(MemoryService::new()));
    register_kernel_service_locked(&mut registry, Box::new(ConsoleService::new()));
}
```

### 注册用户态服务

用户态服务进程启动后注册自己：

```rust
// 用户态服务进程
fn main() {
    // 创建消息队列
    ipc::create_queue(my_tid);
    
    // 注册服务名
    service::register_user_service("fs:/", my_tid).unwrap();
    
    // 服务循环
    loop {
        let msg = ipc::recv(None, None).unwrap();
        handle_request(msg);
    }
}
```

### 服务发现

通过名称查找服务：

```rust
// 查找服务
let service_id = service::lookup_service("fs:/");  // 返回 ServiceId

// 或通过 IPC 模块查找
let tid = ipc::lookup_service("fs:/");  // 返回 TID

// 检查是否为内核服务
if ipc::is_kernel_service(tid) {
    // 使用 service::dispatch 路由到内核服务
} else {
    // 直接发送 IPC 消息到用户态服务
}
```

## IPC 调用约定

### 寄存器传递

| 寄存器 | 用途 |
|--------|------|
| RAX | 固定为 0（IPC_CALL） |
| RDI | 服务 ID |
| RSI | 载荷指针 |
| RDX | 保留 (0) |
| R10 | 消息类型 |
| R8 | 载荷指针 (备用) |
| R9 | 载荷大小 |
| RAX | 返回值 |

### 返回值

- **成功**: 返回值在 RAX 中
- **失败**: RAX 最高位为 1，低 63 位为错误码

```c
if (result >> 63) {
    // 错误: error_code = result & 0x7FFFFFFFFFFFFFFF
} else {
    // 成功: retval = result
}
```

## 服务定义

### 服务 ID

```rust
pub enum ServiceId {
    Process = 0,   // 进程管理
    Memory = 1,    // 内存管理
    Console = 2,   // 控制台 I/O
    FileSystem = 3, // 文件系统
    Device = 4,    // 设备管理
}
```

### 消息格式

```rust
pub struct IpcRequestHeader {
    pub service: u16,      // 目标服务 ID
    pub msg_type: u16,     // 消息类型
    pub flags: u32,        // 标志 (保留)
    pub payload_size: u64, // 载荷大小
}

pub struct IpcResponseHeader {
    pub status: i64,       // 状态码 (0=成功)
    pub retval: u64,       // 返回值
    pub payload_size: u64, // 响应载荷大小
}
```

## ProcessService

服务名: `proc:/`, 服务 ID: `0`

### 消息类型

| 类型 | 值 | 说明 |
|------|-----|------|
| Exit | 0 | 退出当前进程 |
| GetPid | 1 | 获取当前进程 ID |
| Spawn | 2 | 创建新进程 (未实现) |
| Wait | 3 | 等待子进程 (未实现) |

### Exit

```c
// 载荷: 4 字节退出码 (小端序)
uint8_t payload[4] = { code & 0xFF, (code >> 8) & 0xFF, ... };
ipc_call(SERVICE_PROCESS, PROCESS_EXIT, payload, 4);
```

### GetPid

```c
uint64_t pid = ipc_call(SERVICE_PROCESS, PROCESS_GETPID, NULL, 0);
```

## MemoryService

服务名: `mem:/`, 服务 ID: `1`

### 消息类型

| 类型 | 值 | 说明 |
|------|-----|------|
| Mmap | 0 | 映射内存 |
| Munmap | 1 | 取消映射 |
| Brk | 2 | 调整堆边界 |

### Mmap

```c
// 载荷: 32 字节
// [0-7]   addr (u64)
// [8-15]  size (u64)
// [16-23] prot (u64)
// [24-31] flags (u64)

void *mem = (void *)ipc_call(SERVICE_MEMORY, MEMORY_MMAP, payload, 32);
```

**保护标志:**
- `PROT_READ = 0x1`
- `PROT_WRITE = 0x2`
- `PROT_EXEC = 0x4`

**映射标志:**
- `MAP_PRIVATE = 0x02`
- `MAP_ANONYMOUS = 0x20`

### Munmap

```c
// 载荷: 16 字节
// [0-7]  addr (u64)
// [8-15] size (u64)

ipc_call(SERVICE_MEMORY, MEMORY_MUNMAP, payload, 16);
```

### Brk

```c
// 载荷: 8 字节
// [0-7] new_brk (u64)

uint64_t brk = ipc_call(SERVICE_MEMORY, MEMORY_BRK, payload, 8);
```

## ConsoleService

服务名: `console:/`, 服务 ID: `2`

### 消息类型

| 类型 | 值 | 说明 |
|------|-----|------|
| Putc | 0 | 输出字符 |
| Getc | 1 | 输入字符 (未实现) |
| Write | 2 | 输出字符串 |
| Read | 3 | 读取字符串 (未实现) |

### Putc

```c
uint8_t payload[1] = { (uint8_t)c };
ipc_call(SERVICE_CONSOLE, CONSOLE_PUTC, payload, 1);
```

### Write

```c
ipc_call(SERVICE_CONSOLE, CONSOLE_WRITE, str, strlen(str));
```

## 错误码

```rust
pub mod error {
    pub const EINVAL: i64 = 22;      // 无效参数
    pub const ENOSYS: i64 = 38;      // 功能未实现
    pub const ESRCH: i64 = 3;        // 进程不存在
    pub const ENOMEM: i64 = 12;      // 内存不足
    pub const ESRV: i64 = 100;       // 无效服务
    pub const ESRVUNAVAIL: i64 = 101; // 服务不可用
}
```

## 示例

```c
#include "ipc_test.c"

void _start(void) {
    // 输出字符串
    console_write("Hello, World!\n");
    
    // 获取 PID
    uint64_t pid = proc_getpid();
    
    // 分配内存
    void *mem = mem_alloc(4096);
    
    // 退出
    proc_exit(0);
}
```

## 相关文件

| 文件 | 功能 |
|------|------|
| `syscall/mod.rs` | IPC 调用入口 |
| `service/mod.rs` | 服务注册和分发 |
| `service/types.rs` | IPC 类型定义 |
| `service/process.rs` | 进程服务 |
| `service/memory.rs` | 内存服务 |
| `service/console.rs` | 控制台服务 |
| `ipc/mod.rs` | IPC 消息传递、服务注册 |
