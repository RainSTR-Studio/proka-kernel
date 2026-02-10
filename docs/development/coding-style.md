# 代码规范与约定

本文档定义了 Proka Kernel 项目的编码规范、命名约定和代码风格。所有贡献者都应遵守这些约定以保持代码库的一致性和可维护性。


### 代码格式

所有 Rust 代码必须遵循 **Rustfmt** 标准格式：

```bash
make fmt
```

具体规则：
- 4 个空格缩进（无制表符）
- 行宽限制 100 个字符
- 使用 `rustfmt` 默认配置
- 导入按标准库、外部库、内部模块分组排序

### 命名约定

遵循 Rust 社区标准：

| 项目         | 约定                                     | 示例                                         |
| ------------ | ---------------------------------------- | -------------------------------------------- |
| **模块**     | 蛇形命名法（snake_case）                 | `interrupts`, `memory_management`            |
| **结构体**   | 大驼峰命名法（PascalCase）               | `FrameAllocator`, `InterruptDescriptorTable` |
| **枚举**     | 大驼峰命名法（PascalCase）               | `MemoryError`, `DriverType`                  |
| **函数**     | 蛇形命名法（snake_case）                 | `allocate_frame`, `handle_interrupt`         |
| **变量**     | 蛇形命名法（snake_case）                 | `frame_count`, `current_process`             |
| **常量**     | 全大写蛇形命名法（SCREAMING_SNAKE_CASE） | `BASE_REVISION`, `PAGE_SIZE`                 |
| **类型参数** | 大驼峰命名法（PascalCase）               | `T`, `E`, `P`, `Buf`                         |

### 模块组织

```
kernel/src/
├── lib.rs                # 公共 API 导出
├── main.rs               # 内核入口点
└── module_name/
    ├── mod.rs           # 模块声明和内部重导出
    ├── submodule1.rs
    └── submodule2.rs
```

**规则**：
1. 每个目录必须有 `mod.rs` 文件
2. 使用 `pub(crate)` 限制内部可见性
3. 仅在 `lib.rs` 中公开必要的 API
4. 避免模块间的循环依赖

### 注释与文档

对于注释，我们建议使用英文编写，以确保项目的国际化，使全世界的开发者都能够理解和维护代码。

**单行注释**：
```rust
// Use the simple comment to explain the complex logic
let frame = allocator.allocate()?; // If allocation fails, return an error
```

**多行注释**：
```rust
/*
 * Complex algorithm description
 * Second line description
 */
```

**文档注释**：
```rust
/// Allocate a physical frame
///
/// # Arguments
/// - `allocator`: The frame allocator instance
/// - `count`: The number of frames to allocate
///
/// # Returns
/// Returns the address of the allocated frame, or an error
///
/// # Safety Requirements
/// The caller must ensure that the frame allocator is initialized
///
/// # Examples
/// ```
/// let frame = allocate_frames(&mut allocator, 1)?;
/// ```
pub fn allocate_frames(allocator: &mut FrameAllocator, count: usize) -> Result<FrameAddress, MemoryError> {
    // ...
}
```

**内联汇编注释**：
```rust
// SAFETY: Must ensure that the page table address is valid and properly aligned
unsafe {
    asm!("mov cr3, {}", in(reg) page_table_addr);
}
```

### 内存安全与 unsafe 代码

**安全第一原则**：
- 优先编写安全的 Rust 代码
- `unsafe` 块必须最小化并有充分理由

**unsafe 要求**：
1. 每个 `unsafe` 块必须有 `// SAFETY:` 注释
2. 说明为什么是安全的以及调用者需要满足的条件
3. 验证所有不变量和前置条件

**示例**：
```rust
/// Set the current page table
///
/// # Arguments
/// - `page_table_addr`: The address of the page table to set as the current page table
///
/// # Safety Requirements
/// - `page_table_addr` must point to a valid page table
/// - The page table must have the correct permission bits set
/// - The caller must ensure that no invalid memory will be accessed after this function returns
pub unsafe fn set_page_table(page_table_addr: usize) {
    // SAFETY: The caller must ensure that `page_table_addr` points to a valid page table structure,
    // and that no invalid memory will be accessed after this function returns.
    asm!("mov cr3, {}", in(reg) page_table_addr);
}
```

**全局状态**：
- 使用 `Mutex`、`RwLock` 或 `Atomic` 保护共享状态
- 避免裸的全局变量
- 使用 `lazy_static` 或 `once_cell` 进行初始化

## 贡献与提交约定

### Git 提交规范

遵循 Conventional Commits 规范

### 代码审查标准

**审查要点**：
1. 代码是否符合本规范
2. 是否有充分的测试覆盖
3. 文档是否完整
4. 性能影响是否评估
5. 错误处理是否恰当
6. 安全考虑是否充分

**审查流程**：
1. 创建 Pull Request
2. 至少需要一名核心维护者审查
3. 所有 CI 测试必须通过
4. 解决所有审查意见
5. 获得批准后合并

### 测试要求

**单元测试**：
- Rust: 使用 `#[test_case]` 属性，即在测试函数前添加 `#[test_case]` 宏（**不是 `#[test]`!!!**）

**测试文件位置**：
- Rust 测试与源码在同一文件（使用 `#[cfg(test)]`）

**示例**：
```rust
// 这里假定你是在proka-kernel(lib/main)下写的代码，即kernel/src/lib.rs已经
// mod了你的module。
//
// 此时你便可以直接使用`#[test_case]`宏来编写测试用例。

// 这里定义一个功能
pub struct MyStruct {
    pub field: u32,
}

impl MyStruct {
    /// Initilaize a new `MyStruct` with the given field value.
    pub fn new(field: u32) -> Self {
        Self { field }
    }

    /// Return the value of the `field` field.
    pub fn some_method(&self) -> u32 {
        self.field
    }
}

// 这里编写测试用例
#[cfg(test)]
mod tests {
    use super::*;

    /// Test the `new` method.
    #[test_case]
    fn test_new() {
        let my_struct = MyStruct::new(114514);
        assert_eq!(my_struct.field, 114514);
    }

    /// Test the `some_method` method.
    #[test_case]
    fn test_some_method() {
        let my_struct = MyStruct::new(114514);
        assert_eq!(my_struct.some_method(), 114514);
    }
}
```

---

## 总结

本规范是 Proka Kernel 项目的开发基础。所有贡献者应：

1. **阅读并理解**：在编写代码前仔细阅读本规范
2. **持续遵循**：在开发过程中持续检查是否符合规范
3. **积极反馈**：如发现规范问题，请提出改进建议

规范的目的是提高代码质量、保持一致性并降低维护成本。通过共同遵守这些约定，我们可以构建一个更加健壮和可维护的内核。