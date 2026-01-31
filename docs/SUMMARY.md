# Summary

- [序言](README.md)
    - [项目愿景与特性](introduction/vision.md)
    - [代码规范与约定](introduction/conventions.md)

- [快速开始](getting-started/index.md)
    - [环境配置](getting-started/setup.md)
    - [编译与运行](getting-started/build-and-run.md)
    - [调试指南 (QEMU/GDB)](getting-started/debugging.md)

- [内核架构](architecture/index.md)
    - [引导协议 (Limine)](architecture/boot.md)
    - [内核初始化流程](architecture/initialization.md)
    - [中断与异常处理](architecture/interrupts.md)
        - [GDT 与 IDT](architecture/interrupts/gdt_idt.md)
        - [APIC 中断控制器](architecture/interrupts/apic.md)
    - [内存管理](architecture/memory.md)
        - [物理页分配 (PMM)](architecture/memory/frame_allocator.md)
        - [虚拟内存分页 (VMM)](architecture/memory/paging.md)
        - [内核堆管理 (Heap)](architecture/memory/heap.md)
    - [任务与进程管理](architecture/process.md)
    - [图形输出 (Framebuffer)](architecture/graphics.md)

- [设备驱动](drivers/index.md)
    - [驱动框架概览](drivers/overview.md)
    - [串行端口 (UART)](drivers/serial.md)
    - [输入设备 (键盘/鼠标)](drivers/input.md)
    - [时钟与计时器](drivers/timer.md)

- [文件系统](fs/index.md)
    - [虚拟文件系统 (VFS)](fs/vfs.md)
    - [Initrd 初始内存盘](fs/initrd.md)

- [基础设施](infrastructure/index.md)
    - [配置系统 (Anaxa Builder)](infrastructure/config-system.md)
    - [构建系统 (Makefile)](infrastructure/build-system.md)
    - [CI/CD 与工作流](infrastructure/ci-cd.md)
    - [项目目录结构](infrastructure/structure.md)

- [开发手册](development/index.md)
    - [如何编写新驱动](development/new-driver.md)
    - [内核测试框架](development/testing.md)
    - [贡献指南](development/contributing.md)

- [附录](appendices/index.md)
    - [API 参考](api/proka_kernel/index.html)
    - [术语表](appendices/glossary.md)
