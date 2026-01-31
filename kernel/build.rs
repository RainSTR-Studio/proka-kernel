// Proka Kernel - A kernel for ProkaOS
// Copyright (C) RainSTR Studio 2025, All Rights Reserved.
//
// The build script, which can link the C and ASM code.

// Import some modules
use glob::glob;
use std::path::Path;

fn main() {
    anaxa_builder::BuildHelper::new()
        .expect("Failed to create BuildHelper")
        .with_kconfig_dir(".")
        .with_config_file(".config")
        .build()
        .expect("Failed to build configurations");

    println!("cargo:rustc-link-arg=-Tlinker.ld");
    println!("cargo:rustc-link-arg=-nostdlib");
    println!("cargo:rustc-link-arg=-no-pie");

    // Check the file should link
    let workspace_root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("CARGO_MANIFEST_DIR should be doubly nested in workspace")
        .to_path_buf();
    let obj_dir = workspace_root.join("target/obj");

    if let Ok(paths) = glob(&format!("{}/*.o", obj_dir.display())) {
        for path_result in paths {
            if let Ok(path) = path_result {
                // Get the absolute path
                let absolute_path = path.canonicalize().expect("Failed to canonicalize path");
                println!("cargo:rustc-link-arg={}", absolute_path.display());
            }
        }
    }
}
