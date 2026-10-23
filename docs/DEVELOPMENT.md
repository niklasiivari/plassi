# Development

Use a recent stable Rust toolchain. The desktop workflow pins its toolchain in `.github/workflows/desktop.yml`.

On Debian or Ubuntu, install the native build dependencies:

```sh
sudo apt-get install libxkbcommon-dev libwayland-dev libgl1-mesa-dev libx11-dev
cargo run --release -- gui
```

## Checks

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
rustup target add wasm32-unknown-unknown
cargo clippy --locked --workspace --target wasm32-unknown-unknown -- -D warnings
node --test tests/web_workers.mjs
```

For UI changes, test the affected flow on desktop and phone-sized windows. After changing dependencies, regenerate the notices with `./scripts/notices.sh` (requires `cargo-about`).

## Browser build

Install `wasm32-unknown-unknown` and the `wasm-bindgen` CLI version listed in Cargo.lock. Install Binaryen for optional size optimization, then run:

```sh
./scripts/build-web.sh
python3 -m http.server -d dist 8080
```

The solver runs in a module Web Worker, using an isolated instance of the app's
WebAssembly module. PDF typesetting runs in a separate worker loaded on first
print. Keep `workers.js`, `solver-worker.js` and `typeset-worker.js` beside
`index.html` when serving the build. Unsupported module workers retain cooperative
solver and direct PDF fallbacks. Workers use messages and do not require
cross-origin isolation headers.

The default browser build uses the `web-speed` profile for the app and its
random-number generator. The typesetter retains the size profile. Both generated
Wasm files are checked against Cloudflare's 25 MiB per-file limit. Use
`PLASSI_WEB_PROFILE=web ./scripts/build-web.sh` to produce a compact app build.
When comparing profiles, check fixed-seed, fixed-iteration search results as well
as elapsed time, compressed transfer size, startup and peak memory.

In a Chromium worker benchmark with 128 attendees and 512 wishes, the default
profile completed 6,234 iterations versus 4,837 for the compact profile in a
30-second solve, and 2,065 versus 1,648 in a 10-second improvement from the same
seating. Initial assignments and final scores matched. These measurements used
quiet CPU time and the actual default budgets; short runs can give different
results while WebAssembly warms up.

The app and the PDF renderer are separate WebAssembly modules. Each must fit Cloudflare's 25 MiB file limit. Browser projects use local storage and PDFs are downloaded; native projects and prints are files.

## Command line

```sh
cargo run -- import INPUT.tsv PROJECT.json
cargo run -- validate PROJECT.json
cargo run -- solve PROJECT.json
cargo run -- suggest PROJECT.json
cargo run -- export PROJECT.json         # four PDFs, latest seating option
cargo run -- export-xlsx PROJECT.json    # Excel workbook, latest option
cargo run -- schema                     # project JSON Schema
```

`import` replaces its output file; `solve` appends an option. Export commands accept an optional 1-based option number. The schema is also checked in as [project.schema.json](../project.schema.json).

External edits to an open desktop project are reloaded automatically through file
notifications, with content polling when notifications are unavailable. Browser
projects use storage events and focus/visibility resynchronization. Both platforms
still compare stored content immediately before saving. Undo retains 200 lossless
snapshots, compressed incrementally between frames so saving does not wait for
compression.

For external edits, preserve attendee IDs, validate first, and replace the file
atomically. Invalid edits leave the last valid project visible and prevent the
app from overwriting the invalid file.

## Source layout

- `src/model/`: project data, geometry and validation.
- `src/import.rs`, `src/catalog/`, `src/suggest.rs`: spreadsheet data, categories and wish suggestions.
- `src/seating/`, `src/score.rs`: search and scoring.
- `src/arrange/`, `src/roster.rs`: manual moves and attendee changes.
- `src/app/`: shared desktop and browser UI.
- `src/platform/`: platform-specific storage, files and background work.
- `src/print.rs`, `typeset/`: PDF data and rendering.
- `src/cli.rs`: command-line entry points.
