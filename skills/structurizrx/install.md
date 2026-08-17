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
curl -LO https://github.com/pomali/structurizrx/releases/latest/download/structurizrx-x86_64-unknown-linux-gnu.tar.gz.sha256
# Note: both files are served from the same host. For stronger assurance, cross-check
# the hash against the signed release announcement or a GPG/Sigstore signature if
# published alongside the release.
sha256sum --check structurizrx-x86_64-unknown-linux-gnu.tar.gz.sha256
tar xzf structurizrx-*.tar.gz

# User-local install (no administrator privileges required):
mkdir -p ~/.local/bin
install structurizrx ~/.local/bin/
# Ensure ~/.local/bin is on PATH (add to ~/.bashrc or ~/.zshrc if needed):
# export PATH="$HOME/.local/bin:$PATH"

# System-wide install (requires administrator privileges — only if you trust the binary
# and have verified the checksum above):
# sudo install structurizrx /usr/local/bin/
```

Also available as `.deb`/`.rpm` (Linux) and via winget/Chocolatey (Windows) —
see the [full install guide](https://github.com/pomali/structurizrx/blob/main/site/src/install.md).

Verify: `structurizrx --version`.
