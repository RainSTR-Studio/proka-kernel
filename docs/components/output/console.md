# 日志与控制台

Proka Kernel 提供了一个统一的日志系统，支持多种输出后端，包括 VGA 文本模式、帧缓冲区以及串口。

## 架构

- **Logger 核心**：基于 Rust 的 `log` facade 实现。
- **Console Trait**：定义了底层的字符/字符串输出接口。
- **输出驱动**：
    - VGA Text Mode (Legacy)
    - Early Serial (COM1)
    - FB Console (基于图形帧缓冲区)

## 使用示例

```rust
log::info!("Kernel initialized successfully");
log::warn!("Low memory detected: {} KB left", free_kb);
```

## 待完善内容

- [ ] 滚动缓冲区支持。
- [ ] 不同的日志级别颜色显示。
- [ ] 运行时动态切换输出后端。
