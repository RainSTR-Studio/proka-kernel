# Proka Kernel - A kernel for ProkaOS

[![Rust Nightly](https://img.shields.io/badge/rust-nightly-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License: GPLv3](https://img.shields.io/badge/License-GPLv3-yellow.svg?style=flat-square)](https://opensource.org/license/gpl-3.0)
[![GitHub Stars](https://img.shields.io/github/stars/RainSTR-Studio/proka-kernel?style=flat-square)](https://github.com/RainSTR-Studio/proka-kernel/stargazers)
[![GitHub Issues](https://img.shields.io/github/issues/RainSTR-Studio/proka-kernel?style=flat-square)](https://github.com/RainSTR-Studio/proka-kernel/issues)
[![GitHub Pull Requests](https://img.shields.io/github/issues-pr/RainSTR-Studio/proka-kernel?style=flat-square)](https://github.com/RainSTR-Studio/proka-kernel/pulls)
[![Documentation](https://img.shields.io/badge/docs-prokadoc-brightgreen?style=flat-square)](https://prokadoc.pages.dev/)

**Copyright (C) 2026 RainSTR Studio. Licensed under GNU GPLv3.**

---

For more documentation, please visit: [https://prokadoc.pages.dev/](https://prokadoc.pages.dev/).

Welcome to use Proka Kernel, an operating system kernel developed by young talents at RainSTR Studio.
Primarily for learning and practice, our goal is to evolve it into a stable and reliable system.

## Project Highlights
*   **Unique Architecture**: Designed with self-developed architecture (experimental).
*   **Modern Development**: Uses advanced tools and practices for quality code.
*   **Community-Driven**: Actively maintained by passionate developers.

## Languages Used

Proka Kernel is primarily written in **Rust**.

**The Rust Programming Language** forms the kernel's core, offering memory safety to enhance stability and security by preventing common errors. Most high-level logic and new features are in Rust.

## How to Build Proka Kernel?

### Requirements

To build and run Proka Kernel, install these components:

#### Core Build Tools
*   **Rust Toolchain**: `nightly` channel with `x86_64-unknown-none` target (Rust `>= 1.77.0`).
*   **Python**: Build script interpreting support.

#### Optional tools
*   **QEMU**: For kernel emulation.
*   **hadris-fat**: For INITPRT creation.

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
sudo apt-get install python3

# Install Kernel Config Generator
cargo install cargo-anaxa

# Optional: Install optional tools
sudo apt-get install qemu-system-x86
cargo install hadris-fat-cli
```

### Build Process

From the project root:

1.  **Make up config (Optional)**:
    ```bash
    ./build.py menuconfig
    ```
    Please follow its instructions to configure the kernel's settings.

2.  **Compile Kernel**:
    ```bash
    # For release mode
    ./build.py
    
    # For debug mode
    ./build.py --profile dev
    ```
    The kernel file will be put at `output/kernel` in project root.

> [!NOTE]
> This project is based on self-developed bootloader. Since this project is in early development, the structure of files is strictly fixed.
>
> But don't worry, the [ProkaOS Main Repository](https://github.com/RainSTR-Studio/proka-os)'s build script can help us intergrate them, so that you can directly start simulator by passing an ISO file.
> 
> Consequently, if you want to run the whole operating system, consider cloning [ProkaOS Main Repository](https://github.com/RainSTR-Studio/proka-os) and follow its steps in README.

## Contributors
Thank you to all contributors!

*   **zhangxuan2011** <zx20110412@outlook.com>
*   **moyan** <me@moyanjdc.top>
*   **xiaokuai** <rainyhowcool@outlook.com>
*   **LKBaka** <linkervb@outlook.com>

### How to Contribute

We welcome your contributions: Bug reports, Pull Requests (features, fixes, optimizations), documentation improvements, and feedback.

Refer to `CONTRIBUTING.md` for guidelines.

And don't forget to add your name to [**Contributors List**](#contributors)!

## License

Proka Kernel is distributed under the [**GPLv3 License**](LICENSE).
See `LICENSE` file for details.

---
