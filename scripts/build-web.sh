#!/usr/bin/env sh
# Builds the browser version into dist/, a static site for any web host
# (e.g. Cloudflare Pages, which allows 25 MiB per file):
#
#   index.html, plassi.js, plassi_bg.wasm        the app
#   typeset/plassi_typeset*.{js,wasm}            printing, loaded on first print
#
# Needs: rustup target add wasm32-unknown-unknown; a wasm-bindgen CLI of the
# same version as the wasm-bindgen crate in Cargo.lock; wasm-opt (binaryen)
# is used when installed.
set -eu
cd "$(dirname "$0")/.."
target=wasm32-unknown-unknown

# Check the complete toolchain before building or removing the previous site.
if ! command -v wasm-bindgen >/dev/null; then
  echo "error: wasm-bindgen is required (match the version in Cargo.lock)" >&2
  exit 1
fi
expected_bindgen=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)
actual_bindgen=$(wasm-bindgen --version | awk '{print $2}')
if [ "$actual_bindgen" != "$expected_bindgen" ]; then
  echo "error: wasm-bindgen $actual_bindgen does not match Cargo.lock ($expected_bindgen)" >&2
  exit 1
fi
if ! rustup target list --installed | grep -qx "$target"; then
  echo "error: Rust target $target is not installed; run: rustup target add $target" >&2
  exit 1
fi

cargo build --profile web --target $target -p plassi -p plassi-typeset
rm -rf dist
wasm-bindgen --target web --no-typescript --out-dir dist --out-name plassi \
  target/$target/web/plassi.wasm
wasm-bindgen --target web --no-typescript --out-dir dist/typeset --out-name plassi_typeset \
  target/$target/web/plassi_typeset.wasm
if command -v wasm-opt >/dev/null; then
  # Both files at once. Only features every current browser has.
  pids=""
  for wasm in dist/plassi_bg.wasm dist/typeset/plassi_typeset_bg.wasm; do
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
      --enable-mutable-globals --enable-reference-types --enable-multivalue --disable-compact-imports "$wasm" -o "$wasm" &
    pids="$pids $!"
  done
  for pid in $pids; do wait "$pid"; done
fi
cp web/index.html web/favicon.svg dist/
cp THIRD_PARTY_NOTICES.md dist/
ls -l dist/*.wasm dist/typeset/*.wasm
# Cloudflare Pages refuses files over 25 MiB.
for wasm in dist/plassi_bg.wasm dist/typeset/plassi_typeset_bg.wasm; do
  size=$(wc -c < "$wasm")
  if [ "$size" -gt 26214400 ]; then
    echo "error: $wasm is $size bytes, over the 25 MiB file limit" >&2
    exit 1
  fi
done
echo "Built dist/. Try it: python3 -m http.server -d dist 8080"
