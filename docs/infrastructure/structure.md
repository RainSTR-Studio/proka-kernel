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
├── tests/              # 集成测试 (C/Rust)
├── Makefile            # 根目录 Makefile (总控)
└── book.toml           # mdBook 配置文件
```

更多细节请参考各目录下的 `AGENTS.md` 文件。
