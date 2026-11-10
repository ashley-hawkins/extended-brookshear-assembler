#!/bin/sh
set -e
cd "$(dirname "$0")"

# Build with DWARF (the workspace release profile strips it), then keep the full file as the
# debug-symbols copy and ship a stripped one. Needs `cargo binstall wasm-tools` and `uv`.
# Resolve a browser stack trace with: wasm-tools addr2line debug-symbols/<file>.wasm 0xOFFSET...
CARGO_UNSTABLE_BUILD_STD=std,panic_abort \
CARGO_UNSTABLE_BUILD_STD_FEATURES=optimize_for_size \
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=limited \
trunk build --release --public-url './'

hash=$(git rev-parse --short HEAD)
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then hash="$hash-dirty"; fi
mkdir -p debug-symbols
wasm=dist/brookshear_ui_bg.wasm
cp "$wasm" "debug-symbols/brookshear_ui_bg-$hash.wasm"
wasm-tools strip --all "$wasm" -o "$wasm.stripped"
mv "$wasm.stripped" "$wasm"

./deploy.sh
