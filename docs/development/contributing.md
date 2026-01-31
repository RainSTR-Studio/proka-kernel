# 贡献指南

感谢你抽出时间为 Proka Kernel 做出贡献！❤️

## 如何参与

### 提问
在提问之前，请先搜索已有的 [Issues](https://github.com/RainSTR-Studio/proka-kernel/issues)。如果没能找到答案，欢迎提交新的 Issue。

### 报告 Bug
好的 Bug 报告应包含：
- 预期的行为与实际行为。
- 复现步骤。
- 操作系统、平台以及相关工具的版本。

### 提交功能建议
功能建议也通过 GitHub Issues 进行跟踪。请提供清晰的标题和详细的描述，解释为什么该功能对大多数用户有用。

## 开发环境搭建
请参考[环境配置](../getting-started/setup.md)章节。

## 代码质量与格式
我们使用 `pre-commit` 来确保代码质量和格式的一致性。

1. **安装 pre-commit**。
2. **初始化 Hook**：
   ```bash
   uvx pre-commit install
   ```
3. **自动检查**：每次 `git commit` 时都会运行自动检查，包括 Rust 格式化等。

## 提交 PR 的流程
1. **Fork** 仓库并从 `main` 分支创建你的特性分支。
2. **实现** 更改并确保符合现有风格。
3. **测试** 更改（使用 `make run`）。
4. **提交** 更改。
5. **推送** 到你的 Fork 并提交 **Pull Request**。
