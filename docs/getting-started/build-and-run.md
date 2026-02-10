# 编译与运行

## 编译内核
```bash
# 如果是debug（dev）模式的话，直接写make all即可
make all

# 如果是release（prod）模式的话，需要给PROFILE=release。
PROFILE=release make all
```

## 创建 ISO 镜像
```bash
# 与上同理，debug（dev）模式直接写make iso即可，release（prod）模式需要给PROFILE=release。
# 这里以release模式为例，哪个模式下封装哪个模式的镜像。
PROFILE=release make iso
```

## 在 QEMU 中运行
```bash
# 为了调试，使用此命令时将直接以debug（dev）模式运行。
make run
```
