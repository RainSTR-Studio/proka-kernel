# 系统调用接口设计

系统调用是用户空间程序与内核交互的主要接口。Proka Kernel 通过 `syscall` 指令实现系统调用。

## 系统调用机制

### 调用约定

系统调用使用以下寄存器传递参数：

| 寄存器 | 用途 |
|--------|------|
| RAX | 系统调用号 |
| RDI | 参数 1 |
| RSI | 参数 2 |
| RDX | 参数 3 |
| R10 | 参数 4 |
| R8 | 参数 5 |
| R9 | 参数 6 |
| RAX | 返回值 |

### 系统调用号

```rust
pub mod nr {
    pub const EXIT: u64 = 0;       // 进程退出
    pub const PUTC: u64 = 1;       // 字符输出（调试用）
    pub const IPC_SEND: u64 = 2;   // IPC 发送
    pub const IPC_RECV: u64 = 3;   // IPC 接收
    pub const GET_PID: u64 = 4;    // 获取进程 ID
    pub const MMAP: u64 = 5;       // 内存映射
    pub const MUNMAP: u64 = 6;     // 取消映射
    pub const BRK: u64 = 7;        // 调整堆边界
}
```

## 进程管理系统调用

### sys_exit - 进程退出

```c
void exit(int status);
```

终止当前进程，返回退出状态码。

### sys_get_pid - 获取进程 ID

```c
pid_t getpid(void);
```

返回当前进程的进程 ID。

## IPC 系统调用

### sys_ipc_send - 发送消息

```c
int ipc_send(tid_t target, const message_t *msg);
```

向目标线程发送 IPC 消息。

### sys_ipc_recv - 接收消息

```c
int ipc_recv(tid_t *sender, uint64_t timeout_ms, message_t *buffer);
```

接收 IPC 消息，可设置超时。

## 内存管理系统调用

### sys_mmap - 内存映射

```c
void *mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset);
```

将文件或设备映射到内存，或创建匿名映射。

**参数说明：**

| 参数 | 说明 |
|------|------|
| addr | 建议的映射地址（可为 NULL） |
| length | 映射长度（字节） |
| prot | 保护标志 |
| flags | 映射标志 |
| fd | 文件描述符（匿名映射时为 -1） |
| offset | 文件偏移量 |

**保护标志 (prot)：**

```c
#define PROT_NONE   0x0  // 不可访问
#define PROT_READ   0x1  // 可读
#define PROT_WRITE  0x2  // 可写
#define PROT_EXEC   0x4  // 可执行
```

**映射标志 (flags)：**

```c
#define MAP_SHARED    0x01   // 共享映射
#define MAP_PRIVATE   0x02   // 私有映射
#define MAP_FIXED     0x10   // 固定地址
#define MAP_ANONYMOUS 0x20   // 匿名映射
```

**返回值：** 成功返回映射地址，失败返回 `(void *)-1`。

**示例：**

```c
// 分配 4KB 可读写内存
void *mem = mmap(NULL, 4096, PROT_READ | PROT_WRITE, 
                 MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
if (mem == (void *)-1) {
    // 处理错误
}

// 使用完毕后释放
munmap(mem, 4096);
```

### sys_munmap - 取消映射

```c
int munmap(void *addr, size_t length);
```

取消指定地址范围的内存映射。

**返回值：** 成功返回 0，失败返回 -1。

### sys_brk - 调整堆边界

```c
void *brk(void *addr);
```

调整进程堆的边界（program break）。

**参数：**
- `addr = 0`：返回当前堆边界
- `addr > 0`：设置新的堆边界

**返回值：** 新的堆边界地址，失败返回 `(void *)-1`。

## 系统调用实现

### 分发机制

```rust
pub fn dispatch(syscall_num: u64, args: &SyscallArgs) -> u64 {
    match syscall_num {
        nr::EXIT => handlers::sys_exit(args),
        nr::MMAP => handlers::sys_mmap(args),
        // ...
        _ => ENOSYS, // 未实现的系统调用
    }
}
```

### 错误码

```c
#define ENOSYS  38  // 功能未实现
#define EINVAL  22  // 无效参数
```

## 调试系统调用

### sys_putc - 字符输出

```c
void putc(char c);
```

输出单个字符到串口（仅用于调试）。

## 相关文件

| 文件 | 功能 |
|------|------|
| `syscall/mod.rs` | 系统调用入口 |
| `syscall/table.rs` | 系统调用分发表 |
| `syscall/handlers.rs` | 系统调用处理函数 |
| `syscall/mem.rs` | 用户指针验证 |
