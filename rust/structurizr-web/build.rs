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

    // The docs site (`site/` at the repo root) is built separately with
    // `mdbook build site` — this script never invokes mdBook. It only makes
    // sure `site/book/` exists so the `DocsAssets` rust-embed derive doesn't
    // fail to compile before the book has been built for the first time; the
    // `/docs` route serves a setup message when this directory is empty.
    let docs_out_dir = PathBuf::from(&manifest_dir).join("../../site/book");
    std::fs::create_dir_all(&docs_out_dir).ok();

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

    // wasm-pack always builds in release mode, shelling out to its own
    // `cargo build --target wasm32-unknown-unknown`. Cargo's build-directory
    // lock is keyed by profile name only (not by target triple), so if this
    // outer build is itself `--release`, that nested cargo invocation
    // contends for the exact same `target/release/.cargo-lock` this build
    // script's own `cargo build` is holding — a self-deadlock. Point the
    // nested build at its own target directory to avoid sharing the lock.
    let wasm_target_dir = PathBuf::from(&manifest_dir).join("../target/wasm-pack");

    let result = Command::new("wasm-pack")
        .args([
            "build",
            wasm_crate.to_str().unwrap(),
            "--target",
            "web",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .env("CARGO_TARGET_DIR", &wasm_target_dir)
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
