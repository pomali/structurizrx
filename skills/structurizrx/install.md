# Installing `structurizrx`

Only needed if `structurizrx --version` fails. Pick the first option that works
for the platform; all install the same single self-contained binary.

## Fastest paths

- **macOS / Linux (Homebrew):**
  ```sh
  brew install pomali/structurizrx/structurizrx
  ```
- **Windows (Scoop):**
  ```powershell
  scoop bucket add structurizrx https://github.com/pomali/structurizrx
  scoop install structurizrx
  ```
- **Any platform with a Rust toolchain (build from source):**
  ```sh
  git clone https://github.com/pomali/structurizrx
  cargo install --path structurizrx/rust/structurizr-cli
  ```

## Prebuilt binary (no package manager)

Download the archive for the platform from the
[latest release](https://github.com/pomali/structurizrx/releases/latest),
extract, and put `structurizrx` on `PATH`:

| Platform | Asset |
|---|---|
| macOS arm64 | `structurizrx-aarch64-apple-darwin.tar.gz` |
| macOS x86-64 | `structurizrx-x86_64-apple-darwin.tar.gz` |
| Linux x86-64 (glibc) | `structurizrx-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86-64 (static musl) | `structurizrx-x86_64-unknown-linux-musl.tar.gz` |
| Linux arm64 | `structurizrx-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86-64 | `structurizrx-x86_64-pc-windows-msvc.zip` |

```sh
# Linux/macOS example — swap the asset name for the matching platform
curl -LO https://github.com/pomali/structurizrx/releases/latest/download/structurizrx-x86_64-unknown-linux-gnu.tar.gz
tar xzf structurizrx-*.tar.gz
sudo install structurizrx /usr/local/bin/
```

Also available as `.deb`/`.rpm` (Linux) and via winget/Chocolatey (Windows) —
see the [full install guide](https://github.com/pomali/structurizrx/blob/main/site/src/install.md).

Verify: `structurizrx --version`.
