# ELF 加载器

为了运行用户态程序，内核需要能够解析并加载 ELF64 格式的可执行文件。

## 处理步骤

1. **头部解析**：验证魔数 (0x7F 'E' 'L' 'F') 和架构信息。
2. **段映射 (Segments)**：遍历程序头表 (Program Header Table)，将 `PT_LOAD` 段映射到进程的虚拟地址空间。
3. **栈分配**：为用户态初始化栈空间。
4. **跳转入场**：设置 CPU 寄存器并跳转到 Entry Point。

## 待完善内容

- [ ] 动态链接器 (Interpreters) 的基础支持。
- [ ] 辅助向量 (Auxiliary Vector) 的传递。
