#!/usr/bin/env bash
set -eE

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
root_dir=$(cd "${script_dir}/../" && pwd -P)

# Clean previous packages
if [ -d "pkg" ]; then
    rm -rf pkg
fi

BASE_NAME="tycho_emulator"

cd "$root_dir"

# Build for both targets
CRATE="$root_dir/core"
WASM_DIR="$root_dir/src/wasm"
wasm-pack build "$CRATE" --release -t nodejs -d "$WASM_DIR" --out-name "$BASE_NAME"

rm -rf "$WASM_DIR/package.json"
