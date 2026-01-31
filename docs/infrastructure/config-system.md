# 配置系统 (Anaxa Builder)

Proka Kernel 借鉴了 Linux 内核的配置思路，使用 `Kconfig.toml` 文件定义内核的可配置选项，并提供类似 `menuconfig` 的交互界面。我们的配置系统基于 [Anaxa Builder](https://github.com/RainSTR-Studio/anaxa-builder)，一个用于动态配置生成的 Rust 库(没错，也是我们开发的！)。

## 配置定义

我们的根配置文件为`kernel/Kconfig.toml`，该配置文件定义了内核各个子系统的分配置文件引用。

如：
```toml
{{#include ../../kernel/Kconfig.toml}}
```

`menu`下的配置项会生成一个菜单项，其显示名称会优先使用其引用的文件的`title`字段，否则使用他的key。

### 配置项定义

每一个配置项均为`[[config]]`的子项，其完整示例定义如下：
```toml
[[config]]
name = "PAGE_SIZE"  # 配置项名称(必填)
type = "int"        # 配置项类型(必填)
default = 4096      # 默认值(选填)
desc = "The Page Size (Unit: Bytes)." # 描述(选填)
help = "The size of per page."  # 帮助信息(选填)
rust_type = "usize" # Rust 类型(选填)
options = ["RR", "FIFO", "CFS"] # 选项(仅适用于 choice 类型)
depends_on = "PAG_STRATEGY || PAGE_REC" # 依赖项(选填)
range = [1, 1024] # 取值范围(仅适用于 int 类型)
regex = "^[a-zA-Z0-9_]+$" # 正则表达式(仅适用于 string 类型)
feature = ["paas"] # 对应需启用的cargo feature(仅适用于 bool 类型，选填)
```

`type`字段定义了配置项的类型，目前支持以下类型：
- `bool`: 布尔型，值为`true`或`false`
- `int`: 整数型，值为任意整数
- `string`: 字符串型，值为任意字符串
- `hex`: 十六进制型，值为任意十六进制数
- `choice`: 选择型，值为用户选择的选项

`rust_type`字段定义了配置项在 Rust 中的类型，他会覆盖`type`字段的默认映射，但是，`rust_type`字段将会进行验证，如果类型不匹配，将会报错。以下为配置项类型的默认映射：
- `bool`: `bool`
- `int`: `i64`
- `string`: `&str`
- `hex`: `u64`
- `choice`: `&str`


`desc`字段定义了配置项的描述，将会在菜单中显示，并且也会作为其生成的Rust代码的文档字符串。`help`将在TUI的配置项详细页中显示。

`depends_on`字段定义了配置项的依赖项表达式（[`evalexpr`](https://crates.io/crates/evalexpr)语法），如果依赖项表达式求值结果为`true`，则当前配置项将显示。

`range`字段定义了配置项的取值范围(使用`>=`或`<=`)，仅适用于`int`类型。
`regex`字段定义了配置项的匹配正则表达式，仅适用于`string`类型。


### 配置引用定义

```toml
[[menu]]
a = "./a"
```

`a`下的`Kconfig.toml`文件将作为子配置项引用，并自动添加到配置文件中，从而绕过自动的按文件结构构建配置树。

## 条件编译

每一个类型为`bool`的配置项，都会向rustc传递与其名称相同的cfg选项，因此你可以在代码中通过`#[cfg(xxx)]`来进行条件编译，不应使用`#[cfg(feature = "xxx")]`进行条件编译，`feature`只应控制条件依赖。

## 配置检查

配置检查直接使用`make check`，他会同时运行`cargo check`和`cargo anaxa config-check`，以检查配置项的合法性和依赖关系是否正确。

## TUI配置界面

配置界面使用`make menuconfig`启动，会启动一个TUI界面，用户可以通过键盘输入来配置内核。

配置界面的详细使用方法请参考[略]()。
