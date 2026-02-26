# 内存管理架构

Proka Kernel 采用分层内存管理架构，支持内核空间和用户空间的内存隔离与管理。

## 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                    用户空间 (User Space)                      │
│  0x0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF (128TB)           │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ 栈 (Stack) - 向下增长                                │    │
│  │ mmap 区域 (Memory Mappings)                          │    │
│  │ 堆 (Heap) - 向上增长                                 │    │
│  │ 程序段 (Text/Data/BSS)                               │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                    内核空间 (Kernel Space)                    │
│  0xFFFF_8000_0000_0000 - 0xFFFF_FFFF_FFFF_FFFF (128TB)      │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ HHDM (Higher Half Direct Mapping)                    │    │
│  │ 内核堆 (Kernel Heap)                                 │    │
│  │ 内核代码/数据 (Kernel Text/Data/BSS)                  │    │
│  │ ELF 加载区域 (ELF Loading Region)                     │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## 物理内存管理

### 帧分配器 (Frame Allocator)

使用 Buddy System 算法管理物理内存帧：

- **位置**: `kernel/src/memory/frame/buddy.rs`
- **特点**: 支持连续物理内存分配，高效处理内存碎片
- **接口**: `allocate_frame()`, `deallocate_frame()`

```rust
// 获取全局帧分配器
let mut frame_allocator = FRAME_ALLOCATOR;

// 分配单个帧
let frame = frame_allocator.allocate_frame()?;

// 释放帧
frame_allocator.deallocate_frame(frame);
```

## 虚拟内存管理

### VmArea 抽象

VmArea (Virtual Memory Area) 表示一段连续的虚拟内存区域：

```rust
pub struct VmArea {
    pub start: VirtAddr,      // 起始地址
    pub end: VirtAddr,        // 结束地址（不包含）
    pub flags: PageTableFlags, // 页表权限标志
    pub area_type: VmAreaType, // 区域类型
}

pub enum VmAreaType {
    Text,    // 代码段
    Rodata,  // 只读数据
    Data,    // 数据段
    Heap,    // 堆
    Stack,   // 栈
    Mmap,    // 内存映射
    KernelText, KernelRodata, KernelData, KernelBss, KernelHeap,
}
```

### MemorySet

MemorySet 管理一个地址空间的所有 VMA 和页表：

```rust
pub struct MemorySet {
    areas: Vec<VmArea>,
    page_table: OffsetPageTable<'static>,
}
```

## 用户空间内存管理

### 用户空间内存布局

```
地址范围                              用途
────────────────────────────────────────────────
0x0000_0000_0000 - 0x0000_000F_FFFF   Null guard (1MB)
0x0000_0010_0000 - ...                程序代码/数据
0x0000_1000_0000 - 0x0000_7F9F_FFFF   用户堆 (向上增长)
0x0000_7FA0_0000 - 0x0000_7FFF_FFFF   mmap 区域 (256MB)
0x0000_7FC0_0000 - 0x0000_8000_0000   用户栈 (向下增长, 4MB)
```

### 创建用户地址空间

```rust
// 创建新的用户进程地址空间
let memory_set = MemorySet::new_user(&mut frame_allocator)?;
```

### 内存映射 API

#### mmap_anon - 匿名内存映射

```rust
/// 在指定地址映射匿名内存
pub fn mmap_anon(
    &mut self,
    addr: Option<VirtAddr>,  // 首选地址（None 表示自动选择）
    size: usize,              // 映射大小
    flags: PageTableFlags,    // 权限标志
) -> Result<VirtAddr, MemoryError>
```

#### munmap - 取消映射

```rust
/// 取消内存映射并释放物理帧
pub fn munmap(
    &mut self,
    addr: VirtAddr,
    size: usize,
) -> Result<(), MemoryError>
```

### 堆管理

```rust
/// 扩展用户堆
pub fn expand_user_heap(&mut self, new_end: VirtAddr) -> Result<(), MemoryError>

/// 收缩用户堆
pub fn shrink_user_heap(&mut self, new_end: VirtAddr) -> Result<(), MemoryError>

/// 获取当前堆边界
pub fn heap_break(&self) -> VirtAddr
```

## 内核 ELF 加载区域

为支持内核态动态库加载，预留了专用的 ELF 加载区域：

```
起始地址: 0xFFFF_FFFF_8100_0000
大小: 1GB
分配方式: Bump 分配器
```

## 页表管理

### 页表标志

常用标志组合：

| 用途 | 标志 |
|------|------|
| 内核代码 | `PRESENT` |
| 内核数据 | `PRESENT \| WRITABLE \| NO_EXECUTE` |
| 用户代码 | `PRESENT \| USER_ACCESSIBLE` |
| 用户数据 | `PRESENT \| USER_ACCESSIBLE \| WRITABLE \| NO_EXECUTE` |

### TLB 刷新

修改页表后需要刷新 TLB：

```rust
x86_64::instructions::tlb::flush_all();
```

## 错误处理

```rust
pub enum MemoryError {
    AreaOverlap,           // 区域重叠
    AreaNotFound,          // 区域未找到
    FrameAllocationFailed, // 帧分配失败
    MappingFailed,         // 映射失败
}
```

## 相关文件

| 文件 | 功能 |
|------|------|
| `memory/frame/mod.rs` | 帧分配器接口 |
| `memory/frame/buddy.rs` | Buddy System 实现 |
| `memory/paging/mod.rs` | 分页支持 |
| `memory/paging/vmm.rs` | 虚拟内存管理 |
| `memory/heap.rs` | 内核堆管理 |
| `memory/error.rs` | 内存错误类型 |
