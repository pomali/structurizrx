# Install

## Homebrew

```sh
brew tap pomali/structurizrx https://github.com/pomali/structurizrx
brew install structurizrx
```

## Build from source

Requires a Rust toolchain.

```sh
git clone https://github.com/pomali/structurizrx
cd structurizrx/rust
cargo build --release -p structurizr-cli    # binary: target/release/structurizrx
```

All Rust code lives under `rust/` as a Cargo workspace; `cargo` commands
should be run from there. See the repository's `CLAUDE.md` for the full crate
breakdown if you're contributing to StructurizrX itself.

## Verify the install

```sh
structurizrx --version
structurizrx docs   # prints the DSL cheat sheet
```

Next: the [Quickstart](./quickstart.md).
