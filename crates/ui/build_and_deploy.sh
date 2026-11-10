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

# Serve the symbols from dist/symbols/ as <name>-<hash>.wasm.br, brotli-compressed (raw they exceed
# Cloudflare Pages' 25 MiB per-file limit). assets/_redirects and assets/_headers make them
# available at <name>-<hash>.wasm. The app never loads them; the crash-report bot does.
# Older builds are kept in debug-symbols/ and re-copied, so they stay available. Dirty builds are
# not served: crash reports from them cannot be matched to a commit.
mkdir -p dist/symbols
for f in debug-symbols/*.wasm; do
    [ -e "$f.br" ] || uv run --no-project --with brotli==1.2.1 python -c 'import brotli, sys; open(sys.argv[1] + ".br", "wb").write(brotli.compress(open(sys.argv[1], "rb").read(), quality=11, lgwin=24))' "$f"
done
for f in debug-symbols/*.wasm.br; do
    case "$f" in *-dirty.wasm.br) ;; *) cp "$f" dist/symbols/ ;; esac
done

./deploy.sh
