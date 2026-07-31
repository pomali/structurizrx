#!/usr/bin/env bash
# Full docs site build: regenerate example diagrams, then build the mdBook.
# Output lands in site/book/. Requires `mdbook` on PATH.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

"$SCRIPT_DIR/render-examples.sh"
mdbook build "$SCRIPT_DIR"
