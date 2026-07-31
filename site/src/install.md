# Install

## Homebrew (macOS & Linux)

Works on macOS (Apple Silicon and Intel) and Linux (x86-64 and arm64):

```sh
brew tap pomali/structurizrx https://github.com/pomali/structurizrx
brew install pomali/structurizrx/structurizrx
```

## Windows

### Scoop

[Scoop](https://scoop.sh/) installs from the project's bucket:

```powershell
scoop bucket add structurizrx https://github.com/pomali/structurizrx
scoop install structurizrx
```

`scoop update structurizrx` picks up new releases automatically.

### winget

Once the package is published to the
[Windows Package Manager Community Repository](https://github.com/microsoft/winget-pkgs):

```powershell
winget install Pomali.StructurizrX
```

### Chocolatey

Once the package is published to the
[Chocolatey Community Repository](https://community.chocolatey.org/):

```powershell
choco install structurizrx
```

> **Maintainer note:** the winget and Chocolatey manifests live under
> `packaging/winget/` and `packaging/chocolatey/` and are regenerated (version +
> checksum) by the release workflow on each tag. Availability via `winget
> install` / `choco install` additionally requires a one-time submission to each
> catalog (a PR to `microsoft/winget-pkgs`, and `choco push` to the community
> feed), which are external, moderated publishing steps.

## Linux packages (apt / dnf)

Every [release](https://github.com/pomali/structurizrx/releases/latest) also
publishes `.deb` and `.rpm` packages for x86-64 and arm64. These are standalone
package files (not a hosted repository), so download the one matching your
distribution and CPU, then install it with your package manager:

Debian / Ubuntu (`.deb`) — amd64 or arm64:

```sh
VERSION=0.1.1
curl -LO https://github.com/pomali/structurizrx/releases/download/v${VERSION}/structurizrx_${VERSION}_amd64.deb
sudo apt install ./structurizrx_${VERSION}_amd64.deb
```

Fedora / RHEL / openSUSE (`.rpm`) — x86_64 or aarch64:

```sh
VERSION=0.1.1
curl -LO https://github.com/pomali/structurizrx/releases/download/v${VERSION}/structurizrx-${VERSION}-1.x86_64.rpm
sudo dnf install ./structurizrx-${VERSION}-1.x86_64.rpm
```

Because these aren't served from an apt/dnf repository, `apt`/`dnf` won't
auto-update them — grab the newer package on each release (or use Homebrew,
which does track updates).

## Prebuilt binaries

Every [release](https://github.com/pomali/structurizrx/releases/latest)
ships a self-contained `structurizrx` binary for the platforms below. Download
the archive for your platform, extract it, and put the binary on your `PATH`.

| Platform | Download |
|---|---|
| macOS (Apple Silicon) | `structurizrx-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `structurizrx-x86_64-apple-darwin.tar.gz` |
| Linux (x86-64, glibc) | `structurizrx-x86_64-unknown-linux-gnu.tar.gz` |
| Linux (x86-64, static/musl) | `structurizrx-x86_64-unknown-linux-musl.tar.gz` |
| Linux (arm64) | `structurizrx-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86-64) | `structurizrx-x86_64-pc-windows-msvc.zip` |

The static **musl** build has no shared-library dependencies and runs on any
Linux distribution (including Alpine and minimal containers).

### Linux / macOS

```sh
# pick the archive matching your platform from the table above
curl -LO https://github.com/pomali/structurizrx/releases/latest/download/structurizrx-x86_64-unknown-linux-gnu.tar.gz
tar xzf structurizrx-x86_64-unknown-linux-gnu.tar.gz
sudo install structurizrx /usr/local/bin/    # or move it anywhere on your PATH
```

### Windows

Download `structurizrx-x86_64-pc-windows-msvc.zip` from the
[latest release](https://github.com/pomali/structurizrx/releases/latest),
extract `structurizrx.exe`, and place it in a directory on your `PATH` (for
example, run in PowerShell):

```powershell
Expand-Archive structurizrx-x86_64-pc-windows-msvc.zip -DestinationPath .
# then move structurizrx.exe somewhere on your PATH
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
