//! Build script for structurizr-web.
//!
//! If `wasm-pack` is installed this script compiles `structurizr-wasm` to a
//! browser-ready ES-module bundle and writes the output into
//! `assets/wasm/`.  `rust-embed` then picks those files up at compile time so
//! the web server can serve them at `/static/wasm/…`.
//!
//! If `wasm-pack` is not installed the build still succeeds; the canvas demo
//! page will display a setup message instead of a rendered diagram.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let wasm_crate = PathBuf::from(&manifest_dir).join("../structurizr-wasm");
    let out_dir = PathBuf::from(&manifest_dir).join("assets/wasm");

    // Ensure the directory exists so rust-embed doesn't error when it is empty.
    std::fs::create_dir_all(&out_dir).ok();

    let wasm_output = out_dir.join("structurizr_wasm_bg.wasm");

    // Only declare rerun triggers once the output already exists.
    // While it is absent the build script re-runs on every `cargo build`
    // until wasm-pack is available and succeeds.
    if wasm_output.exists() {
        println!(
            "cargo:rerun-if-changed={}",
            wasm_crate.join("src/lib.rs").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            wasm_crate.join("Cargo.toml").display()
        );
    }

    let result = Command::new("wasm-pack")
        .args([
            "build",
            wasm_crate.to_str().unwrap(),
            "--target",
            "web",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .status();

    match result {
        Ok(status) if status.success() => {}
        Ok(status) => {
            println!(
                "cargo:warning=wasm-pack exited with {status}: \
                 the canvas demo at /workspace/<name>/canvas will be unavailable"
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "cargo:warning=wasm-pack not found; the canvas demo at \
                 /workspace/<name>/canvas will display setup instructions. \
                 Install wasm-pack: https://rustwasm.github.io/wasm-pack/"
            );
        }
        Err(e) => {
            println!("cargo:warning=wasm-pack failed to start: {e}");
        }
    }
}
