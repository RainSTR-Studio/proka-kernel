# Proka Kernel - A kernel for ProkaOS

**Copyright (C) 2026 RainSTR Studio. All rights reserved.**

---

Documentation: [https://prokadoc.pages.dev/](https://prokadoc.pages.dev/)

Welcome to Proka Kernel, an operating system kernel developed by young talents at RainSTR Studio.
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
# curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# source $HOME/.cargo/env
# rustup default nightly
# rustup target add x86_64-unknown-none

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
    Please follow its instructions to config.

2.  **Compile Kernel**:
    ```bash
    make
    ```
    Kernel file at `kernel/kernel`.

3.  **Build ISO Image**:
    ```bash
    make iso
    ```
    ISO file `proka-kernel.iso` in project root.

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

### How to Contribute

We welcome your contributions: Bug reports, Pull Requests (features, fixes, optimizations), documentation improvements, and feedback.

Refer to `CONTRIBUTING.md` for guidelines.

And don't forget to add your name to [**Contributors List**](#contributors)!

## License

Proka Kernel is distributed under the [**MIT License**](LICENSE).
See `LICENSE` file for details.

---
