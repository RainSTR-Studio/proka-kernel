# 引导协议 (Limine)

Proka Kernel 严格遵循 [Limine Boot Protocol](https://github.com/limine-bootloader/limine/blob/v5.x-binary/PROTOCOL.md)。

## 关键请求
- **Base Revision**: 内核通过 `BASE_REVISION` 请求告知引导程序其支持的协议版本。
- **Framebuffer Request**: 请求图形帧缓冲区信息。
- **Memory Map Request**: 获取物理内存布局。
