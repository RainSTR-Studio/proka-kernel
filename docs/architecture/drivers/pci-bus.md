# PCI 总线枚举

PCI (Peripheral Component Interconnect) 是内核发现外部硬件驱动的主要入口。

## 枚举流程

1. 从 ACPI 探测 MCFG 表，获取 ECAM 基地址。
2. 扫描所有总线 (Bus)、设备 (Device) 和功能 (Function)。
3. 读取 Vendor ID 和 Device ID，匹配内核驱动。
4. 分配 BAR (Base Address Registers) 资源。

## 中断处理

- 传统引脚中断 (INTx) 的路由。
- MSI 与 MSI-X 的配置。

## 待完善内容

- [ ] PCI 配置空间的抽象接口。
- [ ] 与 PDRM 驱动模型的自动匹配机制。
