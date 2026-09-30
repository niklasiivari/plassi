#!/usr/bin/env bash
# Build command for Cloudflare's Git-connected builds (Workers Builds):
# installs the Rust toolchain and the WebAssembly tools on the build machine,
# then builds the browser version into dist/ (served per wrangler.jsonc).
#
# Tool versions: wasm-bindgen must match the crate in Cargo.lock; binaryen is
# pinned to the version the build was tested with.
set -euo pipefail
cd "$(dirname "$0")/.."

BINARYEN_VERSION=version_133
tools="$HOME/.plassi-tools"
mkdir -p "$tools/bin"
export PATH="$HOME/.cargo/bin:$tools/bin:$PATH"

# Rust, with the WebAssembly target.
if ! command -v cargo >/dev/null; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
rustup target add wasm32-unknown-unknown

# wasm-bindgen CLI of the same version as the crate: a prebuilt release,
# or built from source if the download isn't available.
wasm_bindgen=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3 }' Cargo.lock)
if ! wasm-bindgen --version 2>/dev/null | grep -q "$wasm_bindgen"; then
  name="wasm-bindgen-$wasm_bindgen-x86_64-unknown-linux-musl"
  if curl -sSfL "https://github.com/rustwasm/wasm-bindgen/releases/download/$wasm_bindgen/$name.tar.gz" | tar xz -C "$tools"; then
    cp "$tools/$name/wasm-bindgen" "$tools/bin/"
  else
    cargo install wasm-bindgen-cli --version "$wasm_bindgen" --locked
  fi
fi

# binaryen's wasm-opt, which makes the files small enough for the host.
if ! command -v wasm-opt >/dev/null; then
  curl -sSfL "https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VERSION/binaryen-$BINARYEN_VERSION-x86_64-linux.tar.gz" | tar xz -C "$tools"
  cp "$tools/binaryen-$BINARYEN_VERSION/bin/wasm-opt" "$tools/bin/"
  cp -r "$tools/binaryen-$BINARYEN_VERSION/lib" "$tools/" 2>/dev/null || true
fi

./scripts/build-web.sh
