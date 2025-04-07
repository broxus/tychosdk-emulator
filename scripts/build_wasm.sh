#!/usr/bin/env bash
set -eE

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
root_dir=$(cd "${script_dir}/../" && pwd -P)

function print_help() {
  echo "Usage: ${BASH_SOURCE[0]} [OPTIONS]"
  echo ''
  echo 'Options:'
  echo '  -h,--help         Print this help message and exit'
  echo '  --beta            Build as `-beta` version.'
}

beta=false
while [[ $# -gt 0 ]]; do
  key="$1"
  case $key in
      -h|--help)
        print_help
        exit 0
      ;;
      --beta)
        beta="true"
        shift # past argument
      ;;
      *) # unknown option
        echo 'ERROR: Unknown option'
        echo ''
        print_help
        exit 1
      ;;
  esac
done

# Check if jq is installed
if ! [ -x "$(command -v jq)" ]; then
    echo "jq is not installed" >& 2
    exit 1
fi

# Clean previous packages
if [ -d "pkg" ]; then
    rm -rf pkg
fi

if [ -d "pkg-node" ]; then
    rm -rf pkg-node
fi

SCOPE="tychosdk"
PKG_NAME="emulator-wasm"
if [[ "$beta" == "true" ]]; then
    PKG_NAME="$PKG_NAME-beta"
fi

BASE_NAME="tycho_emulator"

cd "$root_dir"

# Build for both targets
CRATE="$root_dir/core"
wasm-pack build "$CRATE" --release -t nodejs -d "$root_dir/pkg-node" --scope "$SCOPE" --out-name "$BASE_NAME" --features wasm
wasm-pack build "$CRATE" --release -t web -d "$root_dir/pkg" --scope "$SCOPE" --out-name "$BASE_NAME" --features wasm

# Merge nodejs & browser packages
cp "$root_dir/pkg-node/${BASE_NAME}.js" "$root_dir/pkg/${BASE_NAME}_main.js"
cp "$root_dir/pkg-node/${BASE_NAME}.d.ts" "$root_dir/pkg/${BASE_NAME}_main.d.ts"

sed -i -e "s/__wbindgen_placeholder__/wbg/g" "$root_dir/pkg/${BASE_NAME}_main.js"

PACKAGE_JSON=$(
    jq "
      .name = \"@$SCOPE/$PKG_NAME\"
      | .main = \"${BASE_NAME}_main.js\"
      | .browser = \"${BASE_NAME}.js\"
      | .files += [\"${BASE_NAME}_main.js\", \"${BASE_NAME}_main.d.ts\"]
      " \
    "$root_dir/pkg/package.json"
)
echo "$PACKAGE_JSON" > "$root_dir/pkg/package.json"

rm -rf "$root_dir/pkg-node"
