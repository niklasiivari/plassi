#!/usr/bin/env sh
# Regenerates THIRD_PARTY_NOTICES.md: licences of every crate compiled into
# the desktop app and the browser build (dev-dependencies excluded, see
# about.toml), plus the bundled fonts. Run after changing dependencies.
#
# Needs: cargo install cargo-about --locked --features cli
set -eu
cd "$(dirname "$0")/.."
# Do not truncate the checked-in notices if either generation step fails.
data=$(mktemp)
notices=$(mktemp)
trap 'rm -f "$data" "$notices"' EXIT HUP INT TERM
cargo about generate --workspace --format json > "$data"
python3 scripts/notices.py < "$data" > "$notices"
cat "$notices" > THIRD_PARTY_NOTICES.md
echo "Wrote THIRD_PARTY_NOTICES.md"
