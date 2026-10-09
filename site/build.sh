#!/usr/bin/env bash
# Full docs site build: regenerate example diagrams, then build the mdBook,
# then export the interactive demo viewer(s) into it.
# Output lands in site/book/. Requires `mdbook` on PATH.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

"$SCRIPT_DIR/render-examples.sh"
mdbook build "$SCRIPT_DIR"

# Each site/demo/<name>.dsl becomes a static viewer at book/demo/<name>/.
# Exported after `mdbook build` (which empties book/), with the binary
# render-examples.sh just built. structurizr-web's DocsAssets embed excludes
# demo/, so these never end up inside the structurizrx binary.
CLI="$SCRIPT_DIR/../rust/target/release/structurizrx"
for dsl in "$SCRIPT_DIR"/demo/*.dsl; do
    name="$(basename "$dsl" .dsl)"
    "$CLI" export-viewer "$dsl" --output "$SCRIPT_DIR/book/demo/$name"
done
