# 项目目录结构

Proka Kernel 的代码组织遵循模块化原则。

```text
.
├── assets/             # 引导配置、initrd 资源及固件 (OVMF)
├── docs/               # mdBook 文档源码
├── kernel/             # 内核核心源码 (Rust)
│   ├── src/            # 源代码
│   │   ├── drivers/    # 硬件驱动
│   │   ├── memory/     # 内存管理 (Paging, Allocator)
│   │   ├── interrupts/ # IDT, GDT, APIC
│   │   └── ...
│   └── Makefile        # 内核特定构建逻辑
├── scripts/            # 开发与构建辅助脚本
├── Makefile            # 根目录 Makefile (总控)
└── book.toml           # mdBook 配置文件
```

## 内核核心源码

内核核心源码位于 `kernel` 目录下。Rust项目所必须的`Cargo.toml`同样再次目录下，因此需在`kernel`目录下执行`cargo`命令（部分命令已通过`makefile`进行了封装）。

`src`目录下的模块包含：
- `drivers`: 硬件驱动模块
- `memory`: 内存管理模块
- `interrupts`: 中断模块
- `fs`: 文件系统模块
- `process`: 进程模块
- `libs`: 通用工具及杂项模块

## Scripts

脚本位于 `scripts` 目录下。用于存储构建与开发过程中所使用的脚本。一般为`bash`或`python`脚本。该目录不应含任何可执行二进制文件。

- `test_runner.sh`：测试运行器，用于在QEMU中运行内核测试
- `font`: 该目录下为已弃用的`BMF`格式字体生成脚本及其文件格式定义，因为该格式已不再使用。