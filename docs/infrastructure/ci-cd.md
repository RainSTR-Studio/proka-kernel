# CI/CD 与工作流

我们使用 GitHub Actions 来确保代码质量。

## 自动化流水线
每当有代码推送到 `main` 分支或提交 PR 时，CI 会触发以下流程：
- **代码检查**：运行 `cargo fmt` 和 `cargo clippy`。
- **内核构建**：尝试编译 `x86_64-unknown-none` 目标。
- **测试运行**：在 headless 模式下运行自动化测试脚本。

## 配置文件
流水线定义在 `.github/workflows/test.yml`。
