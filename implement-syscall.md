# 实现系统调用 (Syscall) 支持

## TL;DR

> **目标**: 为 Proka Kernel 实现 x86_64 系统调用机制，支持 Ring 3 到 Ring 0 的特权级切换。
>
> **核心交付物**:
> - MSR 配置模块 (IA32_STAR, IA32_LSTAR, IA32_FMASK)
> - syscall/sysret 汇编入口/出口处理
> - 系统调用分发表和处理器
> - 5 个基础系统调用 (exit, putc, ipc_send, ipc_recv, get_pid)
> - GDT 用户态段配置
> - 测试验证程序
>
> **Estimated Effort**: Medium-Large
> **Parallel Execution**: YES - 4 waves
> **Critical Path**: Wave 1 (MSR/GDT) → Wave 2 (Entry/Handler) → Wave 3 (Syscalls) → Wave 4 (Test)

---

## Context

### 当前状态
- Proka Kernel 已具备完整的进程/线程管理 (TCB/PCB)
- IPC 框架已实现，支持消息传递和服务注册
- 调度器支持 Round-Robin 和 Priority 算法
- **缺失**: 用户态执行环境，无法运行 Ring 3 代码

### 架构需求
根据 `docs/architecture/index.md`，系统调用是微内核重构的关键路径：
> "系统调用 (Syscall) 硬件集成：配置 IA32_LSTAR 实现 Ring 3 到 Ring 0 的快速跳转"

### 设计决策

#### 调用约定 (System V AMD64 ABI)
| 寄存器 | 作用 |
|--------|------|
| `rax` | 系统调用号 (输入) / 返回值 (输出) |
| `rdi` | 参数 1 |
| `rsi` | 参数 2 |
| `rdx` | 参数 3 |
| `r10` | 参数 4 (内核端，避免与 rcx 冲突) |
| `r8`  | 参数 5 |
| `r9`  | 参数 6 |
| `rcx` | 硬件保存的返回地址 (rip) |
| `r11` | 硬件保存的 CPU 标志位 (rflags) |

#### MSR 配置
- `IA32_STAR` (0xC0000081): 设置内核/用户代码段
- `IA32_LSTAR` (0xC0000082): syscall 入口点地址
- `IA32_FMASK` (0xC0000084): RFLAGS 掩码 (屏蔽 IF/TF)

#### GDT 布局要求
```
Index  | Selector | 用途
-------|----------|------------------
0      | 0x00     | Null
1      | 0x08     | Kernel Code (64-bit)
2      | 0x10     | Kernel Data
3      | 0x18     | User Data (32-bit compat)
4      | 0x20     | User Code (32-bit compat)
5      | 0x28     | User Data (64-bit)
6      | 0x30     | User Code (64-bit)
7      | 0x38     | TSS
```

#### 基础系统调用表
| 编号 | 名称 | 功能 | 参数 |
|------|------|------|------|
| 0 | sys_exit | 终止当前进程 | code: i32 |
| 1 | sys_putc | 字符输出 (调试) | c: u8 |
| 2 | sys_ipc_send | 发送 IPC 消息 | target: Tid, msg: *const Message |
| 3 | sys_ipc_recv | 接收 IPC 消息 | sender: Option<Tid>, timeout: Option<u64> |
| 4 | sys_get_pid | 获取当前进程 PID | - |

---

## Work Objectives

### Core Objective
实现完整的 x86_64 系统调用机制，使内核能够处理来自 Ring 3 的系统调用请求，并提供基础系统调用服务。

### Concrete Deliverables
1. `kernel/src/syscall/msr.rs` - MSR 读写和配置
2. `kernel/src/syscall/entry.asm` - syscall/sysret 汇编处理
3. `kernel/src/syscall/mod.rs` - 系统调用模块主入口
4. `kernel/src/syscall/table.rs` - 系统调用分发表
5. `kernel/src/syscall/handlers.rs` - 系统调用处理函数
6. `kernel/src/interrupts/gdt.rs` 更新 - 添加用户态段
7. `kernel/src/main.rs` 更新 - 初始化系统调用
8. 测试程序验证 Ring 3 切换

### Definition of Done
- [ ] 系统调用初始化成功，MSR 配置正确
- [ ] 用户态程序能成功触发 syscall 并进入内核
- [ ] 5 个基础系统调用均可正常工作
- [ ] 系统调用返回后用户态程序继续执行
- [ ] 所有测试通过

### Must Have
- MSR 配置 (STAR, LSTAR, FMASK)
- syscall/sysret 汇编处理
- 系统调用分发机制
- 5 个基础系统调用实现
- GDT 用户态段配置

### Must NOT Have (Guardrails)
- 不实现完整的 POSIX 系统调用集
- 不实现复杂的参数验证 (仅基础检查)
- 不实现信号处理
- 不修改现有内核线程机制

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES (自定义测试框架 `#[test_case]`)
- **Automated tests**: YES (Tests-after)
- **Framework**: 自定义 test_runner

### QA Policy
每个任务包含 Agent-Executed QA Scenarios，验证方式：
- **内核代码**: 通过 `make test` 运行单元测试
- **集成测试**: 通过 `make run` 在 QEMU 中验证
- **手动验证**: 检查串口输出和内核日志

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation - MSR & GDT):
├── Task 1: 实现 MSR 读写函数 (wrmsr/rdmsr)
├── Task 2: 添加 MSR 常量定义 (IA32_STAR, IA32_LSTAR, IA32_FMASK)
├── Task 3: 更新 GDT 添加用户态代码/数据段
└── Task 4: 实现 MSR 配置函数 (configure_syscall_msrs)

Wave 2 (Core - Entry & Handler):
├── Task 5: 创建 syscall 汇编入口 (entry.asm)
├── Task 6: 实现寄存器保存/恢复逻辑
├── Task 7: 创建系统调用模块主文件 (mod.rs)
└── Task 8: 实现系统调用分发表

Wave 3 (Implementation - Syscalls):
├── Task 9: 实现 sys_exit 系统调用
├── Task 10: 实现 sys_putc 系统调用
├── Task 11: 实现 sys_ipc_send 系统调用
├── Task 12: 实现 sys_ipc_recv 系统调用
└── Task 13: 实现 sys_get_pid 系统调用

Wave 4 (Integration & Test):
├── Task 14: 在 main.rs 中初始化系统调用
├── Task 15: 创建用户态测试程序框架
├── Task 16: 实现 Ring 3 切换测试
└── Task 17: 运行完整测试验证

Wave FINAL (Review):
├── Task F1: 代码审查和质量检查
├── Task F2: 文档更新
└── Task F3: 最终集成测试

Critical Path: T1→T3→T5→T8→T14→T16→F3
```

### Dependency Matrix

- **T1**: - → T2, T4
- **T2**: - → T4
- **T3**: - → T5, T14
- **T4**: T1, T2 → T14
- **T5**: T3 → T6, T7
- **T6**: T5 → T7
- **T7**: T5, T6 → T8
- **T8**: T7 → T9-T13
- **T9-T13**: T8 → T14
- **T14**: T3, T4, T9-T13 → T15
- **T15**: T14 → T16
- **T16**: T15 → T17
- **T17**: T16 → F1-F3

### Agent Dispatch Summary

- **Wave 1**: 4 tasks → `quick` (MSR/GDT 配置)
- **Wave 2**: 4 tasks → `deep` (汇编入口和分发逻辑)
- **Wave 3**: 5 tasks → `unspecified-high` (系统调用实现)
- **Wave 4**: 4 tasks → `deep` (集成和测试)
- **Wave FINAL**: 3 tasks → `unspecified-high` (审查和文档)

---

## TODOs

- [ ] 1. 实现 MSR 读写函数

  **What to do**:
  - 在 `kernel/src/libs/msr.rs` 添加 `wrmsr` 和 `rdmsr` 内联汇编函数
  - 添加 MSR 常量: IA32_STAR (0xC0000081), IA32_LSTAR (0xC0000082), IA32_FMASK (0xC0000084)
  - 确保函数标记为 `unsafe` 因为 MSR 操作影响系统状态

  **Must NOT do**:
  - 不要添加其他无关的 MSR 常量
  - 不要修改现有 APIC MSR 定义

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: 简单的内联汇编包装，需要精确但代码量少

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 4
  - **Blocked By**: None

  **References**:
  - `kernel/src/libs/msr.rs:1-136` - 现有 MSR 常量定义模式
  - x86_64 手册 Volume 2: MSR 操作指令

  **Acceptance Criteria**:
  - [ ] `wrmsr(msr: u32, value: u64)` 函数实现
  - [ ] `rdmsr(msr: u32) -> u64` 函数实现
  - [ ] IA32_STAR, IA32_LSTAR, IA32_FMASK 常量定义
  - [ ] 代码通过 `make` 编译无错误

  **QA Scenarios**:
  ```
  Scenario: MSR 函数编译测试
    Tool: Bash
    Preconditions: 代码已写入文件
    Steps:
      1. 运行 `cd /home/moyan/projects/proka-kernel && make`
      2. 检查编译输出无错误
    Expected Result: 编译成功，无警告
    Evidence: 终端输出截图
  ```

  **Commit**: YES
  - Message: `feat(syscall): add MSR read/write functions and syscall constants`
  - Files: `kernel/src/libs/msr.rs`

- [ ] 2. 更新 GDT 添加用户态段

  **What to do**:
  - 修改 `kernel/src/interrupts/gdt.rs`
  - 添加用户态数据段和代码段描述符 (Ring 3)
  - 确保 GDT 布局符合 syscall/sysret 要求:
    - 内核代码段: 0x08 (CS)
    - 内核数据段: 0x10 (DS)
    - 用户数据段: 0x18 (兼容模式)
    - 用户代码段: 0x20 (兼容模式)
    - 用户数据段 (64-bit): 0x28
    - 用户代码段 (64-bit): 0x30
  - 添加选择器常量供其他模块使用

  **Must NOT do**:
  - 不要修改内核段的选择器值 (保持向后兼容)
  - 不要删除 TSS 段

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: GDT 配置是静态结构修改，需要精确但逻辑简单

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 5, Task 14
  - **Blocked By**: None

  **References**:
  - `kernel/src/interrupts/gdt.rs:1-57` - 现有 GDT 实现
  - x86_64 手册 Volume 3: GDT 和段描述符
  - `x86_64::structures::gdt::Descriptor` API 文档

  **Acceptance Criteria**:
  - [ ] 用户态数据段描述符添加 (Ring 3, 64-bit)
  - [ ] 用户态代码段描述符添加 (Ring 3, 64-bit, 可执行)
  - [ ] 选择器常量导出: `USER_CODE_SELECTOR`, `USER_DATA_SELECTOR`
  - [ ] 代码通过 `make` 编译无错误

  **QA Scenarios**:
  ```
  Scenario: GDT 编译和加载测试
    Tool: Bash
    Preconditions: 代码已写入文件
    Steps:
      1. 运行 `cd /home/moyan/projects/proka-kernel && make`
      2. 运行 `make run` 启动内核
      3. 检查内核是否正常启动无 GDT 相关错误
    Expected Result: 内核正常启动，GDT 加载成功
    Evidence: QEMU 输出日志
  ```

  **Commit**: YES
  - Message: `feat(gdt): add user-mode code and data segments`
  - Files: `kernel/src/interrupts/gdt.rs`

- [ ] 3. 实现 MSR 配置函数

  **What to do**:
  - 创建 `kernel/src/syscall/msr.rs` 文件
  - 实现 `configure_syscall_msrs()` 函数:
    - 设置 IA32_STAR: 内核 CS = 0x08, 用户 CS = 0x30 (sysret 会自动减 16/24)
    - 设置 IA32_LSTAR: 指向 syscall_entry (稍后定义)
    - 设置 IA32_FMASK: 屏蔽 IF (0x200) 和 TF (0x100) 位
  - 添加 `enable_syscall()` 函数设置 EFER.SCE 位

  **Must NOT do**:
  - 不要硬编码入口点地址，使用占位符或 extern 声明
  - 不要修改其他 MSR

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: 配置逻辑直接，主要是 MSR 位操作

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 1)
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 14
  - **Blocked By**: Task 1

  **References**:
  - `kernel/src/libs/msr.rs` - MSR 读写函数 (Task 1)
  - x86_64 手册 Volume 2: SYSCALL/SYSRET 指令
  - EFER MSR (0xC0000080) - SCE (Syscall Enable) 位

  **Acceptance Criteria**:
  - [ ] `configure_syscall_msrs(entry_point: u64)` 函数实现
  - [ ] `enable_syscall()` 函数实现 (设置 EFER.SCE)
  - [ ] 正确的 MSR 值计算和设置
  - [ ] 代码通过 `make` 编译无错误

  **QA Scenarios**:
  ```
  Scenario: MSR 配置函数编译测试
    Tool: Bash
    Preconditions: Task 1 完成
    Steps:
      1. 运行 `cd /home/moyan/projects/proka-kernel && make`
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES (与 Task 1 合并)
  - Message: `feat(syscall): add MSR configuration functions`
  - Files: `kernel/src/syscall/msr.rs`

- [ ] 4. 创建 syscall 汇编入口

  **What to do**:
  - 创建 `kernel/src/syscall/entry.asm` 文件
  - 实现 `syscall_entry` 标签:
    - 使用 `swapgs` 切换到内核 GS 基址
    - 保存用户态 rsp 到 scratchpad，加载内核栈
    - 保存所有通用寄存器到栈
    - 准备参数 (寄存器 → 函数调用约定)
    - 调用 Rust 处理函数
  - 实现 `syscall_exit` 标签:
    - 恢复寄存器
    - 恢复用户态 rsp
    - `swapgs`
    - `sysretq`

  **Must NOT do**:
  - 不要遗漏任何寄存器保存 (尤其是 rcx, r11 被硬件修改)
  - 不要修改中断状态 (syscall 会自动禁用中断)

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []
  - **Reason**: 汇编代码需要精确的寄存器管理和栈操作，容易出错

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 3 的 GDT)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 6, Task 7
  - **Blocked By**: Task 3

  **References**:
  - `kernel/src/interrupts/gdt.rs` - 内核栈和 GDT 配置
  - `kernel/src/process/context_switch.rs` - 上下文切换模式
  - x86_64 手册 Volume 2: SYSCALL/SYSRET 指令行为
  - Linux 内核 entry_64.S 参考实现

  **Acceptance Criteria**:
  - [ ] `syscall_entry` 汇编标签实现
  - [ ] `syscall_exit` 汇编标签实现
  - [ ] 正确的寄存器保存/恢复顺序
  - [ ] 栈切换逻辑正确
  - [ ] 与 Rust 代码正确链接

  **QA Scenarios**:
  ```
  Scenario: 汇编入口编译测试
    Tool: Bash
    Preconditions: Task 3 完成
    Steps:
      1. 运行 `make`
      2. 检查汇编文件被正确编译和链接
    Expected Result: 编译成功，无链接错误
    Evidence: 终端输出
  ```

  **Commit**: YES
  - Message: `feat(syscall): add syscall/sysret assembly entry/exit`
  - Files: `kernel/src/syscall/entry.asm`

- [ ] 5. 创建系统调用模块主文件

  **What to do**:
  - 创建 `kernel/src/syscall/mod.rs`
  - 定义 `SyscallArgs` 结构体保存寄存器参数
  - 实现 `syscall_handler(args: SyscallArgs) -> u64` 函数
  - 集成 MSR 配置和汇编入口
  - 实现 `init()` 初始化函数
  - 在 `kernel/src/lib.rs` 添加 `pub mod syscall`

  **Must NOT do**:
  - 不要直接处理系统调用逻辑 (留给 table.rs)
  - 不要暴露内部实现细节

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []
  - **Reason**: 需要整合多个组件，设计清晰的模块接口

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 4, Task 5)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 8
  - **Blocked By**: Task 4, Task 5

  **References**:
  - `kernel/src/syscall/entry.asm` - 汇编入口 (Task 5)
  - `kernel/src/syscall/msr.rs` - MSR 配置 (Task 3)
  - `kernel/src/ipc/mod.rs` - 模块设计参考

  **Acceptance Criteria**:
  - [ ] `SyscallArgs` 结构体定义 (rax, rdi, rsi, rdx, r10, r8, r9)
  - [ ] `syscall_handler()` 函数框架
  - [ ] `init()` 初始化函数调用 MSR 配置
  - [ ] 模块导出正确的公共接口
  - [ ] `lib.rs` 更新包含新模块

  **QA Scenarios**:
  ```
  Scenario: 模块编译和集成测试
    Tool: Bash
    Preconditions: Task 4, Task 5 完成
    Steps:
      1. 运行 `make`
      2. 检查模块是否正确链接
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES
  - Message: `feat(syscall): add syscall module main file`
  - Files: `kernel/src/syscall/mod.rs`, `kernel/src/lib.rs`

- [ ] 6. 实现系统调用分发表

  **What to do**:
  - 创建 `kernel/src/syscall/table.rs`
  - 定义 `SyscallHandler` 类型别名: `fn(&SyscallArgs) -> u64`
  - 创建 `SYSCALL_TABLE: [Option<SyscallHandler>; 5]` 静态数组
  - 实现 `dispatch(syscall_num: u64, args: &SyscallArgs) -> u64` 函数
  - 处理无效系统调用号 (返回 -ENOSYS)

  **Must NOT do**:
  - 不要使用过大的表 (限制在 5 个基础调用)
  - 不要动态分配内存

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: 简单的查找表实现

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 7)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 9-13
  - **Blocked By**: Task 7

  **References**:
  - `kernel/src/syscall/mod.rs` - SyscallArgs 定义 (Task 7)
  - Linux 内核系统调用表实现模式

  **Acceptance Criteria**:
  - [ ] `SyscallHandler` 类型定义
  - [ ] `SYSCALL_TABLE` 静态数组 (大小 5)
  - [ ] `dispatch()` 函数实现边界检查
  - [ ] 无效调用返回 -38 (ENOSYS)

  **QA Scenarios**:
  ```
  Scenario: 分发表编译测试
    Tool: Bash
    Preconditions: Task 7 完成
    Steps:
      1. 运行 `make`
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES (与 Task 7 合并)
  - Message: `feat(syscall): add syscall dispatch table`
  - Files: `kernel/src/syscall/table.rs`

- [ ] 7. 实现 sys_exit 系统调用

  **What to do**:
  - 在 `kernel/src/syscall/handlers.rs` 实现 `sys_exit(args: &SyscallArgs) -> u64`
  - 从 args.rdi 获取退出码
  - 调用 `process::terminate_current(exit_code)` (需要创建此函数)
  - 此调用不返回 (线程终止)

  **Must NOT do**:
  - 不要直接操作 PCB/TCB，使用现有 API
  - 不要忘记清理资源

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []
  - **Reason**: 需要理解进程终止流程

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 8)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 14
  - **Blocked By**: Task 8

  **References**:
  - `kernel/src/process/scheduler.rs` - 线程终止 API
  - `kernel/src/process/process.rs` - 进程终止逻辑
  - `kernel/src/syscall/table.rs` - 注册到表中的位置 0

  **Acceptance Criteria**:
  - [ ] `sys_exit()` 函数实现
  - [ ] 正确获取退出码参数
  - [ ] 调用调度器终止当前线程
  - [ ] 在 SYSCALL_TABLE[0] 注册

  **QA Scenarios**:
  ```
  Scenario: sys_exit 编译测试
    Tool: Bash
    Preconditions: Task 8 完成
    Steps:
      1. 运行 `make`
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES
  - Message: `feat(syscall): implement sys_exit syscall`
  - Files: `kernel/src/syscall/handlers.rs`

- [ ] 8. 实现 sys_putc 系统调用

  **What to do**:
  - 实现 `sys_putc(args: &SyscallArgs) -> u64`
  - 从 args.rdi 获取字符 (u8)
  - 使用 `serial_print!` 或 `print!` 输出字符
  - 返回 0 表示成功

  **Must NOT do**:
  - 不要直接操作硬件端口
  - 不要阻塞

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: 简单的输出包装

  **Parallelization**:
  - **Can Run In Parallel**: YES (与 Task 9 并行)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 14
  - **Blocked By**: Task 8

  **References**:
  - `kernel/src/output/serial.rs` - 串口输出
  - `kernel/src/libs/logger.rs` - 日志输出

  **Acceptance Criteria**:
  - [ ] `sys_putc()` 函数实现
  - [ ] 正确输出字符到串口
  - [ ] 返回 0
  - [ ] 在 SYSCALL_TABLE[1] 注册

  **QA Scenarios**:
  ```
  Scenario: sys_putc 功能测试
    Tool: Bash
    Preconditions: 代码完成
    Steps:
      1. 运行 `make test`
    Expected Result: 测试通过
    Evidence: 测试输出
  ```

  **Commit**: YES (与 Task 9 合并)
  - Message: `feat(syscall): implement sys_putc syscall`
  - Files: `kernel/src/syscall/handlers.rs`

- [ ] 9. 实现 sys_ipc_send 系统调用

  **What to do**:
  - 实现 `sys_ipc_send(args: &SyscallArgs) -> u64`
  - 参数: rdi = target_tid, rsi = msg_ptr (用户态指针)
  - 验证用户态指针 (检查地址范围)
  - 调用 `ipc::send(target, msg, true)`
  - 返回 0 或错误码

  **Must NOT do**:
  - 不要直接解引用用户态指针而不验证
  - 不要在内核态分配大内存

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []
  - **Reason**: 涉及用户态指针验证和 IPC 调用

  **Parallelization**:
  - **Can Run In Parallel**: YES (与 Task 9, 10 并行)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 14
  - **Blocked By**: Task 8

  **References**:
  - `kernel/src/ipc/mod.rs` - IPC send 函数
  - `kernel/src/syscall/handlers.rs` - 指针验证模式

  **Acceptance Criteria**:
  - [ ] `sys_ipc_send()` 函数实现
  - [ ] 用户态指针验证
  - [ ] 正确调用 IPC 发送
  - [ ] 在 SYSCALL_TABLE[2] 注册

  **QA Scenarios**:
  ```
  Scenario: sys_ipc_send 编译测试
    Tool: Bash
    Preconditions: 代码完成
    Steps:
      1. 运行 `make`
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES
  - Message: `feat(syscall): implement sys_ipc_send syscall`
  - Files: `kernel/src/syscall/handlers.rs`

- [ ] 10. 实现 sys_ipc_recv 系统调用

  **What to do**:
  - 实现 `sys_ipc_recv(args: &SyscallArgs) -> u64`
  - 参数: rdi = sender_tid (0 = any), rsi = timeout_ms (0 = 无限)
  - 调用 `ipc::recv(sender, timeout)`
  - 将接收到的消息写入用户态缓冲区 (需要额外参数)
  - 返回消息大小或错误码

  **Must NOT do**:
  - 不要阻塞内核线程 (使用 IPC 的阻塞机制)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []
  - **Reason**: 需要处理阻塞和缓冲区写入

  **Parallelization**:
  - **Can Run In Parallel**: YES (与 Task 9, 11 并行)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 14
  - **Blocked By**: Task 8

  **References**:
  - `kernel/src/ipc/mod.rs` - IPC recv 函数

  **Acceptance Criteria**:
  - [ ] `sys_ipc_recv()` 函数实现
  - [ ] 正确处理 sender 和 timeout 参数
  - [ ] 消息数据写入用户态缓冲区
  - [ ] 在 SYSCALL_TABLE[3] 注册

  **QA Scenarios**:
  ```
  Scenario: sys_ipc_recv 编译测试
    Tool: Bash
    Preconditions: 代码完成
    Steps:
      1. 运行 `make`
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES (与 Task 11 合并)
  - Message: `feat(syscall): implement sys_ipc_recv syscall`
  - Files: `kernel/src/syscall/handlers.rs`

- [ ] 11. 实现 sys_get_pid 系统调用

  **What to do**:
  - 实现 `sys_get_pid(args: &SyscallArgs) -> u64`
  - 调用 `process::current_pid()`
  - 返回 PID 作为 u64

  **Must NOT do**:
  - 不要添加额外参数检查

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: 最简单的系统调用

  **Parallelization**:
  - **Can Run In Parallel**: YES (与 Task 9-11 并行)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 14
  - **Blocked By**: Task 8

  **References**:
  - `kernel/src/process/mod.rs` - current_pid() 函数

  **Acceptance Criteria**:
  - [ ] `sys_get_pid()` 函数实现
  - [ ] 返回当前进程 PID
  - [ ] 在 SYSCALL_TABLE[4] 注册

  **QA Scenarios**:
  ```
  Scenario: sys_get_pid 编译测试
    Tool: Bash
    Preconditions: 代码完成
    Steps:
      1. 运行 `make`
    Expected Result: 编译成功
    Evidence: 终端输出
  ```

  **Commit**: YES (与 Task 11 合并)
  - Message: `feat(syscall): implement sys_get_pid syscall`
  - Files: `kernel/src/syscall/handlers.rs`

- [ ] 12. 在 main.rs 中初始化系统调用

  **What to do**:
  - 修改 `kernel/src/main.rs`
  - 在 `ipc::init()` 之后添加 `syscall::init()`
  - 确保初始化顺序: GDT → IDT → Memory → Process → Scheduler → IPC → Syscall
  - 添加日志输出确认初始化成功

  **Must NOT do**:
  - 不要改变其他初始化顺序
  - 不要在启用中断前初始化 syscall

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []
  - **Reason**: 简单的初始化调用添加

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 3, Task 7, Task 9-13)
  - **Parallel Group**: Wave 4
  - **Blocks**: Task 15
  - **Blocked By**: Task 3, Task 7, Task 9-13

  **References**:
  - `kernel/src/main.rs:58-63` - 现有初始化顺序
  - `kernel/src/syscall/mod.rs` - init() 函数 (Task 7)

  **Acceptance Criteria**:
  - [ ] `syscall::init()` 在 main 中调用
  - [ ] 正确的初始化顺序
  - [ ] 内核启动日志显示 "Syscall subsystem initialized"
  - [ ] 代码通过 `make` 编译

  **QA Scenarios**:
  ```
  Scenario: 系统调用初始化测试
    Tool: Bash
    Preconditions: 所有前置任务完成
    Steps:
      1. 运行 `make`
      2. 运行 `make run`
      3. 检查串口输出包含 "Syscall subsystem initialized"
    Expected Result: 内核启动，显示初始化成功消息
    Evidence: QEMU 串口输出
  ```

  **Commit**: YES
  - Message: `feat(main): initialize syscall subsystem`
  - Files: `kernel/src/main.rs`

- [ ] 13. 创建用户态测试程序框架

  **What to do**:
  - 创建 `tests/user/` 目录
  - 创建 `tests/user/syscall_test.c` 或 `.rs` 测试程序
  - 实现用户态 syscall 封装函数 (使用 inline asm)
  - 编写测试用例调用各个系统调用
  - 创建 Makefile 或构建脚本编译用户态程序

  **Must NOT do**:
  - 不要依赖标准库 (使用裸机/自定义运行时)
  - 不要编写过于复杂的测试

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []
  - **Reason**: 需要理解用户态环境和系统调用约定

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 14)
  - **Parallel Group**: Wave 4
  - **Blocks**: Task 16
  - **Blocked By**: Task 14

  **References**:
  - x86_64 用户态系统调用内联汇编示例
  - Linux 用户态 syscall 封装实现

  **Acceptance Criteria**:
  - [ ] 用户态测试程序源文件
  - [ ] syscall() 封装函数 (rax=调用号, rdi,rsi,rdx,r10,r8,r9)
  - [ ] 基础测试用例 (exit, putc, get_pid)
  - [ ] 编译脚本/说明

  **QA Scenarios**:
  ```
  Scenario: 测试程序编译
    Tool: Bash
    Preconditions: 代码完成
    Steps:
      1. 按照说明编译用户态测试程序
    Expected Result: 编译成功，生成可执行文件
    Evidence: 终端输出
  ```

  **Commit**: YES
  - Message: `test(syscall): add user-mode test program`
  - Files: `tests/user/syscall_test.c`, `tests/user/Makefile`

- [ ] 14. 实现 Ring 3 切换测试

  **What to do**:
  - 创建内核测试函数模拟 Ring 3 切换
  - 设置用户态栈和上下文
  - 使用 `sysret` 或 IRET 切换到 Ring 3
  - 在用户态执行 `syscall` 指令
  - 验证成功进入内核处理程序
  - 返回内核态并验证状态

  **Must NOT do**:
  - 不要直接在生产代码中测试 (使用测试模块)
  - 不要破坏内核状态

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []
  - **Reason**: 涉及特权级切换，需要精确的上下文管理

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖 Task 15)
  - **Parallel Group**: Wave 4
  - **Blocks**: Task 17
  - **Blocked By**: Task 15

  **References**:
  - `kernel/src/process/context_switch.rs` - 上下文切换
  - x86_64 特权级切换文档
  - `sysret` 指令行为

  **Acceptance Criteria**:
  - [ ] Ring 3 切换测试函数
  - [ ] 成功切换到 Ring 3
  - [ ] 用户态执行 syscall 并进入内核
  - [ ] 正确返回内核态
  - [ ] 测试通过标记

  **QA Scenarios**:
  ```
  Scenario: Ring 3 切换测试
    Tool: Bash
    Preconditions: Task 15 完成
    Steps:
      1. 运行 `make test`
      2. 检查测试输出
    Expected Result: Ring 3 切换测试通过
    Evidence: 测试输出日志
  ```

  **Commit**: YES (与 Task 15 合并)
  - Message: `test(syscall): add Ring 3 transition test`
  - Files: `kernel/src/syscall/test.rs` 或测试模块

- [ ] 15. 运行完整测试验证

  **What to do**:
  - 运行 `make test` 确保所有单元测试通过
  - 运行 `make run` 启动内核验证正常运行
  - 检查串口输出确认系统调用初始化成功
  - 验证没有新的警告或错误
  - 运行 clippy 检查代码质量

  **Must NOT do**:
  - 不要忽略测试失败
  - 不要忽略编译警告

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []
  - **Reason**: 全面的验证和测试

  **Parallelization**:
  - **Can Run In Parallel**: NO (依赖所有前置任务)
  - **Parallel Group**: Wave 4
  - **Blocks**: Wave FINAL
  - **Blocked By**: Task 16

  **References**:
  - `Makefile` - 测试目标
  - `kernel/src/test.rs` - 测试框架

  **Acceptance Criteria**:
  - [ ] `make test` 通过
  - [ ] `make run` 正常启动
  - [ ] 无编译警告
  - [ ] clippy 无严重问题
  - [ ] 串口输出确认系统调用工作

  **QA Scenarios**:
  ```
  Scenario: 完整测试套件
    Tool: Bash
    Preconditions: 所有任务完成
    Steps:
      1. 运行 `make clean && make`
      2. 运行 `make test`
      3. 运行 `make run` 并观察输出
      4. 运行 `cd kernel && cargo clippy`
    Expected Result: 全部通过，无错误
    Evidence: 终端输出和日志
  ```

  **Commit**: NO (验证步骤，不单独提交)

---

## Final Verification Wave

- [ ] F1. **代码审查** - `unspecified-high`
  审查所有 syscall 相关代码:
  - 检查汇编代码的寄存器保存/恢复
  - 验证 MSR 配置值
  - 检查用户态指针验证
  - 确保错误处理正确
  Output: 审查报告，问题列表

- [ ] F2. **文档更新** - `writing`
  更新文档:
  - 在 `docs/architecture/multitasking/syscalls.md` 添加设计文档
  - 更新 `docs/architecture/index.md` 标记系统调用为完成
  - 添加系统调用接口说明
  Output: 更新后的文档文件

- [ ] F3. **最终集成测试** - `deep`
  完整集成测试:
  - 启动内核并验证系统调用初始化
  - 运行用户态测试程序
  - 验证所有 5 个系统调用工作正常
  - 测试错误处理 (无效调用号)
  Output: 测试报告

---

## Commit Strategy

- **Task 1-3**: 合并提交 `feat(syscall): add MSR configuration infrastructure`
- **Task 4**: 单独提交 `feat(syscall): add syscall/sysret assembly entry`
- **Task 5-6**: 合并提交 `feat(syscall): add syscall module and dispatch table`
- **Task 7-11**: 合并提交 `feat(syscall): implement basic syscall handlers`
- **Task 12**: 单独提交 `feat(main): initialize syscall subsystem`
- **Task 13-14**: 合并提交 `test(syscall): add user-mode syscall tests`
- **Task F2**: 单独提交 `docs(syscall): add syscall design documentation`

---

## Success Criteria

### Verification Commands
```bash
# 编译测试
cd /home/moyan/projects/proka-kernel && make clean && make

# 单元测试
make test

# 运行验证
make run

# 代码质量
cd kernel && cargo clippy -- -D warnings
```

### Final Checklist
- [ ] IA32_STAR, IA32_LSTAR, IA32_FMASK 正确配置
- [ ] syscall/sysret 汇编入口工作正常
- [ ] 5 个基础系统调用实现完整
- [ ] 系统调用分发表正确路由
- [ ] 用户态测试程序可以触发 syscall
- [ ] Ring 3 到 Ring 0 切换成功
- [ ] 所有测试通过
- [ ] 文档已更新
- [ ] 无编译警告
