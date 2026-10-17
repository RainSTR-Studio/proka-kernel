//! Build script of Proka Arch-Specified module.
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use std::process::{Command, Stdio};
use std::io::Write;
use anyhow::anyhow;
use std::fs;

fn main() -> anyhow::Result<()> {
    let mut arch_raw = fs::read_to_string("archlist")?;

    // Get each arch name...
    arch_raw.retain(|c| !matches!(c, ' ' | '\n' | '\r' | '\t'));
    let archlist = arch_raw
        .split(',')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    // Generate the Rust code...
    let mut items = TokenStream::new();
    for arch in &archlist {
        let arch_ident = Ident::new(arch, Span::call_site());
        let doc_str = format!("{} specified library.", arch_ident);
        let block = quote! {
            #[doc = #doc_str]
            #[cfg(target_arch = #arch)]
            mod #arch_ident;
            #[cfg(target_arch = #arch)]
            pub use #arch_ident::*;
        };
        items.extend(block);
    }

    // TokenStream -> String and write to file
    let raw_code = items.to_string();
    let header = r##"/* This file is generated. Do not edit manually. */
//! Arch-specified library.
#![no_std]
#![cfg_attr(
    not(all(feature = "output_fb", feature = "output_serial")),
    allow(unused)
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![allow(clippy::empty_loop)]

"##;
    let tail = {
        let mut origin = "#[cfg(not(all(".to_string();
        for arch in archlist {
            origin.push_str(&format!("target_arch = \"{arch}\","));
        }
        origin.push_str(r#")))]
compile_error!("You are trying to compile this project by using an unsupported architecture.");
"#);
        origin
    };

    let final_code = format_code(&raw_code).map_or_else(
        |_| format!("{header}{raw_code}\n\n{tail}"),
        |formatted| format!("{header}{formatted}\n\n{tail}"),
    );

    fs::write("src/lib.rs", final_code)?;

    Ok(())
}

/// Formats a Rust source string by piping it through `rustfmt`.
///
/// # Arguments
/// - `unformatted_code` – Raw Rust source code to format.
///
/// # Returns
/// The formatted source string, or `Err(RustFmtError)` if `rustfmt` is not available
/// or formatting fails.
pub fn format_code(unformatted_code: &str) -> anyhow::Result<String> {
    let child_result = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let Ok(mut child) = child_result else {
        return Err(anyhow!("no child result"));
    };

    // Write the code to rustfmt's stdin
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(unformatted_code.as_bytes()).is_err()
    {
        return Err(anyhow!("stdin error"));
    }

    match child.wait_with_output() {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8(output.stdout)?)
        }
        _ => Err(anyhow!("stdout error")),
    }
}
