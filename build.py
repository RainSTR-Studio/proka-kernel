#!/usr/bin/env python3
# Proka Kernel - Python Build System
# Copyright (C) RainSTR Studio 2025-2026, All Rights Reserved.
# 
# Usage:
#     python3 build.py                     # build kernel in dev mode, same as `make`
#     python3 build.py --profile release   # build kernel in release mode
#     python3 build.py clean               # remove all build artifacts
#     python3 build.py clippy              # run cargo clippy
#     python3 build.py fmt                 # run cargo fmt
#     python3 build.py menuconfig          # run cargo anaxa menuconfig
#     python3 build.py doc                 # build mdBook + rustdoc documentation
#     python3 build.py docs-clean          # clean documentation

import os
import shutil
import subprocess
import sys
import argparse
import logging

# ANSI color codes
BOLD = "\033[1m"
RESET = "\033[0m"
CYAN = "\033[36m"

class CustomFormatter(logging.Formatter):
    def format(self, record):
        if record.levelno == logging.INFO:
            return f"{BOLD}[INFO]{RESET} {record.getMessage()}"
        return super().format(record)

# Configure logging
logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)
handler = logging.StreamHandler()
handler.setFormatter(CustomFormatter())
logger.addHandler(handler)
# Prevent duplicate messages
logger.propagate = False

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
OUT_DIR = os.path.join(BASE_DIR, "output")
OUTPUT = os.path.join(OUT_DIR, "proka-kernel")
PKG_NAME = "proka-kernel"
RUST_TARGET = "x86_64-unknown-none"

# Rust compilation flags (mirrors RUSTFLAGS in the Makefile)
RUSTFLAGS = (
    "-C relocation-model=static "
    "-C code-model=large "
    "-C no-redzone "
    "-C force-frame-pointers=yes"
)


def run(cmd, cwd=BASE_DIR, env=None, check=True):
    """Run a command, log it. Mirrors Make's command echo."""
    logger.info(f"{BOLD}{CYAN}Running:{RESET} {' '.join(cmd)}")
    full_env = os.environ.copy()
    if env:
        full_env.update(env)
    subprocess.run(cmd, cwd=cwd, env=full_env, check=check)


def find_tool(name, fallback):
    """Locate a tool on PATH, falling back to a hard-coded name."""
    path = shutil.which(name)
    return path if path else fallback


def profile_dir(profile):
    """Map build profile to Cargo's output directory name."""
    return "debug" if profile == "dev" else profile


class Builder:
    """Base builder class"""
    def __init__(self):
        raise NotImplementedError

    def clean(self):
        raise NotImplementedError


class Kernel(Builder):
    def __init__(self, profile="dev"):
        self.profile = profile
        self.build()

    def build(self):
        """Compile the kernel and pack the raw binary into output/."""
        logger.info(f"Building kernel in {self.profile} mode...")
        bin_path = os.path.join(
            BASE_DIR, "target", RUST_TARGET,
            profile_dir(self.profile), PKG_NAME,
        )
        run(
            ["cargo", "anaxa", "build", "--no-env",
             "--target", RUST_TARGET, "--profile", self.profile],
            env={"RUSTFLAGS": RUSTFLAGS},
        )

        # Copy the ELF and produce the raw binary
        os.makedirs(OUT_DIR, exist_ok=True)
        elf_path = os.path.join(OUT_DIR, "proka-kernel.elf")
        shutil.copyfile(bin_path, elf_path)
        objcopy = find_tool("objcopy", "objcopy")
        run([objcopy, "-O", "binary", elf_path, OUTPUT])
        os.remove(bin_path)
        logger.info(f"Kernel binary ready: {OUTPUT}")

    def clippy(self):
        run(["cargo", "clippy", "--target", RUST_TARGET, "--all-features"],
            env={"RUSTFLAGS": RUSTFLAGS})

    def menuconfig(self):
        run(["cargo", "anaxa", "menuconfig"])

    def fmt(self):
        run(["cargo", "fmt"])

    def doc(self):
        logger.info("Building guide (mdBook)...")
        run(["mdbook", "build"])
        logger.info("Building API documentation (rustdoc)...")
        run(["cargo", "doc", "--no-deps"])
        book_api = os.path.join(BASE_DIR, "book", "api")
        shutil.rmtree(book_api, ignore_errors=True)
        shutil.copytree(
            os.path.join(BASE_DIR, "target", "doc"),
            book_api,
            dirs_exist_ok=True,
        )
        logger.info("Documentation has successfully built in book")

    def docs_clean(self):
        logger.info("Cleaning documentation...")
        run(["mdbook", "clean"])
        shutil.rmtree(os.path.join(BASE_DIR, "book"), ignore_errors=True)
        run(["cargo", "clean", "--doc"])

    def clean(self):
        self.docs_clean()
        run(["cargo", "clean"])
        shutil.rmtree(OUT_DIR, ignore_errors=True)

def main():
    parser = argparse.ArgumentParser(description="Proka Kernel build script")
    parser.add_argument(
        "target", nargs="?", default="all",
        choices=["all", "clean", "clippy", "fmt", "menuconfig", "doc", "docs-clean"],
        help="Build target (default: all)",
    )
    parser.add_argument(
        "--profile", default="dev", choices=["dev", "release"],
        help="Build profile (default: dev)",
    )
    args = parser.parse_args()

    if args.target == "all":
        Kernel(profile=args.profile)
    elif args.target == "clean":
        Kernel.__new__(Kernel).clean()
    elif args.target == "clippy":
        Kernel.__new__(Kernel).clippy()
    elif args.target == "fmt":
        Kernel.__new__(Kernel).fmt()
    elif args.target == "menuconfig":
        Kernel.__new__(Kernel).menuconfig()
    elif args.target == "doc":
        Kernel.__new__(Kernel).doc()
    elif args.target == "docs-clean":
        Kernel.__new__(Kernel).docs_clean()


if __name__ == "__main__":
    main()
