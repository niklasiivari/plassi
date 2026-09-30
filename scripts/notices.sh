#!/usr/bin/env sh
# Regenerates THIRD_PARTY_NOTICES.md: licences of every crate compiled into
# the desktop app and the browser build (dev-dependencies excluded, see
# about.toml), plus the bundled fonts. Run after changing dependencies.
#
# Needs: cargo install cargo-about --locked --features cli
set -eu
cd "$(dirname "$0")/.."
cargo about generate --workspace --format json 2>/dev/null | python3 scripts/notices.py > THIRD_PARTY_NOTICES.md
echo "Wrote THIRD_PARTY_NOTICES.md"
