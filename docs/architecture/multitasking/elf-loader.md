# ELF 加载器

为了运行用户态程序和加载内核模块，内核需要能够解析并加载 ELF64 格式的可执行文件和共享库。

## 概述

Proka Kernel 使用 `elf_loader` crate 实现 ELF 加载功能，并通过自定义的 `KernelMmap` trait 实现内核态内存映射。

## 处理步骤

1. **头部解析**：验证魔数 (`0x7F 'E' 'L' 'F'`) 和架构信息。
2. **段映射 (Segments)**：遍历程序头表 (Program Header Table)，将 `PT_LOAD` 段映射到地址空间。
3. **重定位**：处理重定位表，修正符号地址。
4. **符号解析**：解析动态符号表，查找符号地址。

## KernelMmap 实现

`KernelMmap` 实现了 `elf_loader::os::Mmap` trait，提供内核态内存映射支持：

### 内存分配区域

ELF 加载使用内核空间专用区域：

```
起始地址: 0xFFFF_FFFF_8100_0000
大小: 1GB
分配方式: Bump 分配器
```

### 接口实现

```rust
impl Mmap for KernelMmap {
    /// 通用内存映射
    unsafe fn mmap(
        addr: Option<usize>,
        len: usize,
        prot: ProtFlags,
        flags: MapFlags,
        offset: usize,
        fd: Option<isize>,
        need_copy: &mut bool,
    ) -> Result<*mut c_void>;
    
    /// 匿名内存映射
    unsafe fn mmap_anonymous(
        addr: usize,
        len: usize,
        prot: ProtFlags,
        flags: MapFlags,
    ) -> Result<*mut c_void>;
    
    /// 取消映射
    unsafe fn munmap(addr: *mut c_void, len: usize) -> Result<()>;
    
    /// 修改内存保护属性
    unsafe fn mprotect(
        addr: *mut c_void,
        len: usize,
        prot: ProtFlags,
    ) -> Result<()>;
    
    /// 预留地址空间
    unsafe fn mmap_reserve(
        addr: Option<usize>,
        len: usize,
        use_file: bool,
    ) -> Result<*mut c_void>;
}
```

### 权限标志转换

```rust
fn prot_to_flags(prot: ProtFlags) -> PageTableFlags {
    let mut flags = PageTableFlags::PRESENT;
    
    if prot.contains(ProtFlags::PROT_WRITE) {
        flags |= PageTableFlags::WRITABLE;
    }
    if !prot.contains(ProtFlags::PROT_EXEC) {
        flags |= PageTableFlags::NO_EXECUTE;
    }
    
    flags
}
```

## 使用示例

### 加载共享库

```rust
use crate::libs::elf::{load_elf, test_load_elf};

// 加载动态库
let lib = load_elf("mylib.so")?;

// 获取符号
let add_func = unsafe { lib.get::<fn(i32, i32) -> i32>("add")? };

// 调用函数
let result = add_func(1, 2);  // 返回 3
```

### 编译共享库

使用 GCC 编译适用于内核加载的共享库：

```bash
# 最小化编译（不链接 libc）
gcc -shared -fPIC -nostdlib -o mylib.so mylib.c

# 示例源码
cat > mylib.c << 'EOF'
int add(int a, int b) {
    return a + b;
}
EOF
```

## 加载流程详解

### 1. 文件读取

```rust
let f = VFS.open(path)?;
let file_data = f.read_all()?;
let data = file_data.as_slice();
```

### 2. 创建加载器

```rust
let mut loader = Loader::new().with_mmap::<KernelMmap>();
```

### 3. 加载和重定位

```rust
let lib = loader
    .load_dylib(ElfBinary::new(path, data))?
    .relocator()
    .relocate()?;
```

## 内核内存集访问

ELF 加载器需要访问内核页表进行映射。由于进程管理器初始化时会获取内核内存集，加载器通过以下方式访问：

```rust
// 优先通过进程管理器获取内核进程的内存集
if let Some(pcb) = process::process::lock().get_process(0) {
    let pcb_lock = pcb.lock();
    let mut ms = pcb_lock.memory_set.lock();
    // 使用页表进行映射...
}
```

## 注意事项

1. **地址空间**：确保使用内核空间的 canonical 地址（`0xFFFF_XXXX_XXXX_XXXX`）
2. **TLB 刷新**：映射完成后必须刷新 TLB
3. **内存对齐**：所有映射地址必须页对齐（4KB）
4. **错误处理**：正确处理内存分配失败的情况

## 相关文件

| 文件 | 功能 |
|------|------|
| `libs/elf.rs` | ELF 加载器和 KernelMmap 实现 |
| `memory/paging/vmm.rs` | 虚拟内存管理 |
| `fs/vfs/mod.rs` | 文件系统接口 |

## 待完善内容

- [ ] 文件映射支持（非匿名映射）
- [ ] 动态链接器 (Interpreters) 支持
- [ ] 辅助向量 (Auxiliary Vector) 传递
- [ ] 符号版本控制

