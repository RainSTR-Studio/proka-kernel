# 配置系统 (Kconfig)

Proka Kernel 借鉴了 Linux 内核的配置思路，使用 `Kconfig.toml` 文件定义内核的可配置选项，并提供类似 `menuconfig` 的交互界面。

## 技术栈
- **cargo-anaxa**: 一个基于 Rust 的内核配置工具，读取 `Kconfig.toml` 并生成 Rust 代码。
- **Kconfig.toml**: 采用 TOML 格式定义配置项及其依赖关系。

## 如何配置
运行以下命令进入 TUI 配置界面：
```bash
make menuconfig
```

## 原理
1. `make menuconfig` 启动 `cargo-anaxa`。
2. 用户在界面中选择配置。
3. 工具生成一个临时的 `.config` 文件（或直接导出）。
4. 在构建过程中，Rust 代码通过 `include!` 宏包含生成的配置常量。
