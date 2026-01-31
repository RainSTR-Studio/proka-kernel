# 构建系统 (Makefile)

项目采用 Makefile 作为顶层调度器，配合 Cargo 完成 Rust 内核的编译。

## 顶层 Makefile
位于项目根目录，负责：
- 调度内核编译。
- 调用 `xorriso` 生成 ISO。
- 管理 `scripts` 下的辅助工具。
- 启动 QEMU。

## 内核 Makefile
位于 `kernel/Makefile`，专注于：
- 处理 `cargo` 编译参数。
- 链接内核二进制文件。

## 构建流程
1. **预处理**：生成配置头文件。
2. **内核编译**：`cargo build` 编译 Rust 源码为 ELF。
3. **Initrd 创建**：打包 `assets` 中的必要文件。
4. **镜像合成**：将 Limine 引导程序、内核 ELF 和 Initrd 合并为 ISO。
