#!/usr/bin/env bash
# Regenerates the SVG diagrams embedded in the docs site (site/src/images/)
# from the example .dsl files in site/examples/, using the structurizrx CLI
# itself. Run this before `mdbook build site` whenever an example changes —
# or just run `site/build.sh`, which does both in order.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT/rust"
cargo build --release -p structurizr-cli
CLI="$REPO_ROOT/rust/target/release/structurizrx"

for dsl in "$SCRIPT_DIR"/examples/*.dsl; do
    name="$(basename "$dsl" .dsl)"
    out_dir="$SCRIPT_DIR/src/images/$name"
    rm -rf "$out_dir"
    mkdir -p "$out_dir"
    "$CLI" render "$dsl" --format svg --output "$out_dir"
done
