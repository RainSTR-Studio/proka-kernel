# Proka Kernel - A kernel for ProkaOS

[![Kernel Tests](https://github.com/RainSTR-Studio/proka-kernel/actions/workflows/test.yml/badge.svg)](https://github.com/RainSTR-Studio/proka-kernel/actions/workflows/test.yml)
[![Rust Nightly](https://img.shields.io/badge/rust-nightly-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License: GPLv3](https://img.shields.io/badge/License-GPLv3-yellow.svg?style=flat-square)](https://opensource.org/license/gpl-3.0)
[![GitHub Stars](https://img.shields.io/github/stars/RainSTR-Studio/proka-kernel?style=flat-square)](https://github.com/RainSTR-Studio/proka-kernel/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/RainSTR-Studio/proka-kernel?style=flat-square)](https://github.com/RainSTR-Studio/proka-kernel/issues)
[![GitHub Pull Requests](https://img.shields.io/github/issues-pr/RainSTR-Studio/proka-kernel?style=flat-square)](https://github.com/RainSTR-Studio/proka-kernel/pulls)
[![Documentation](https://img.shields.io/badge/docs-prokadoc-brightgreen?style=flat-square)](https://prokadoc.pages.dev/)

**Copyright (C) 2026 RainSTR Studio. All rights reserved.**

---

For more documentation, please visit: [https://prokadoc.pages.dev/](https://prokadoc.pages.dev/).

Welcome to use Proka Kernel, an operating system kernel developed by young talents at RainSTR Studio.
Primarily for learning and practice, our goal is to evolve it into a stable and reliable system.

## Project Highlights
*   **Hybrid Language Design**: Leverages Rust's memory safety and C's low-level control.
*   **Modular Architecture**: Designed for clarity and extensibility.
*   **Modern Development**: Uses advanced tools and practices for quality code.
*   **Community-Driven**: Actively maintained by passionate developers.

## Languages Used

Proka Kernel is primarily written in **C and Rust**.

*   **Rust**: Forms the kernel's core, offering memory safety to enhance stability and security by preventing common errors. Most high-level logic and new features are in Rust.
*   **C Language**: Utilized for direct hardware access, low-level operations (e.g., boot code, assembly interfaces), and integrating existing drivers.

## How to Build Proka Kernel?

### Requirements

To build and run Proka Kernel, install these components:

#### Core Build Tools
*   **Rust Toolchain**: `nightly` channel with `x86_64-unknown-none` target (Rust `>= 1.77.0`).
*   **GCC**: C language compiler.
*   **Make**: Build automation tool.

#### Runtime & Image Creation
*   **QEMU**: For kernel emulation.
*   **xorriso**: To create bootable ISO images.
*   **cpio**: To create initrd (initial RAM disk) images.

**Note**: GCC might be pre-installed on your OS.

### Installation Commands

We recommend `rustup` for Rust management.

#### Linux (Debian/Ubuntu)

```bash
# Install Rust (via rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup default nightly
rustup target add x86_64-unknown-none

# Install core build tools
sudo apt-get update
sudo apt-get install -y gcc make

# Install runtime and image creation tools
sudo apt-get install -y xorriso cpio qemu-system-x86

# Install Kernel Config Generator
cargo install cargo-anaxa
```

### Build Process

From the project root:

1.  **Make up config (Optional)**:
    ```bash
    make menuconfig
    ```
    Please follow its instructions to configure the kernel's settings.

2.  **Compile Kernel**:
    ```bash
    make
    ```
    The kernel file will be put at `output/kernel` in project root.

3.  **Build ISO Image**:
    ```bash
    make iso
    ```
    The ISO file will be put at `output/proka-kernel.iso` in project root.

4.  **Run in QEMU**:
    ```bash
    make run
    ```
    QEMU will launch, displaying kernel output in the terminal.


## Contributors
Thank you to all contributors!

*   **zhangxuan2011** <zx20110412@outlook.com>
*   **moyan** <me@moyanjdc.top>
*   **xiaokuai** <rainyhowcool@outlook.com>
*   **TMX** <273761857@qq.com>
*   **LKBaka** <linkervb@outlook.com>

### How to Contribute

We welcome your contributions: Bug reports, Pull Requests (features, fixes, optimizations), documentation improvements, and feedback.

Refer to `CONTRIBUTING.md` for guidelines.

And don't forget to add your name to [**Contributors List**](#contributors)!

## License

Proka Kernel is distributed under the [**GPLv3 License**](LICENSE).
See `LICENSE` file for details.

---
