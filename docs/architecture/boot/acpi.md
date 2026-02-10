# ACPI 探测与解析

ACPI (Advanced Configuration and Power Interface) 是现代 PC 硬件发现和配置的核心。Proka Kernel 使用 ACPI 来枚举系统硬件，特别是多处理器环境下的中断控制器。

## 主要功能

- **RSDP/XSDP 查找**：从 Limine 提供的地址或传统内存区域查找根描述符。
- **MADT (Multiple APIC Description Table) 解析**：获取 CPU 核心信息、Local APIC 地址以及 I/O APIC 配置。
- **FADT (Fixed ACPI Description Table) 解析**：获取电源管理、硬件特征等信息。
- **硬件资源发现**：为 PDRM 提供底层硬件拓扑。

## 待完善内容

- [ ] ACPI 表的校验和验证逻辑。
- [ ] 动态解析 AML (ACPI Machine Language) 的计划。
- [ ] 与 PCI 总线枚举的集成。
