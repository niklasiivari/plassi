# Developing Plassi

## Building and running

```sh
cargo run -- gui          # the desktop app
cargo test                 # unit tests and UI click-through tests
./scripts/build-web.sh     # the browser version, into dist/
./scripts/notices.sh       # regenerate THIRD_PARTY_NOTICES.md after dependency changes
```

Other commands:

```sh
cargo run -- import data/data.tsv plassi.json
cargo run -- validate plassi.json
cargo run -- solve plassi.json
cargo run -- suggest plassi.json      # wishes found in sign-up text
cargo run -- export plassi.json [N]   # the four A4 PDFs for option N (default: latest)
cargo run -- schema > project.schema.json
```

The `import` command replaces the named output project. `solve` appends one candidate to it. [project.schema.json](project.schema.json) is a generated copy of the app's JSON Schema; the `schema` command regenerates it from the Rust types.

## In the browser

`scripts/build-web.sh` builds the same app as a static site in `dist/` (needs `rustup target add wasm32-unknown-unknown`, the `wasm-bindgen` CLI matching `Cargo.lock`, and optionally `wasm-opt` from binaryen). Serve `dist/` from any static host, e.g. Cloudflare Pages; `python3 -m http.server -d dist 8080` tries it locally.

Differences from the desktop app:

- Projects live in the browser's local storage. The project switcher can download one as JSON or load one from a file.
- Searches run on the page's one thread, a slice between frames, so the page stays responsive; the desktop runs one search per CPU core.
- Printing is a separate module (`typeset/`, Typst plus the bundled Source Sans and Source Serif fonts) loaded the first time you print, and PDFs arrive as downloads. It is split off because Typst is most of the size: the app is about 6 MB and the typesetter about 22 MiB, each under Cloudflare Pages' 25 MiB file limit.

The platform differences live in `src/platform/` (`native.rs`, `web.rs`); the rest of the code is shared.

## Code layout

- `src/model/` — the project file format (`mod.rs`), seat geometry (`layout.rs`) and validation (`validate.rs`).
- `src/catalog/` — categories for organizations, tickets, food and drinks; membership and merge suggestions.
- `src/import.rs`, `src/suggest.rs` — spreadsheet import and suggested wishes from sign-up text.
- `src/score.rs` — per-rule results shown in the app.
- `src/seating/` — `problem.rs` (the project compiled to indices, and the score), `spare_seats.rs` (which seats stay empty), `tabu.rs` (the search), `mod.rs` (`solve` / `improve`).
- `src/groups.rs` — wish groups and group-move suggestions.
- `src/roster.rs` — adding, removing and replacing attendees.
- `src/print.rs` — the data behind the A4 prints; `typeset/` (crate `plassi-typeset`) turns it into PDFs with Typst and the bundled fonts.
- `src/platform/` — storage, background work and files, for the desktop and the browser.
- `src/cli.rs` — headless commands.
- `src/app/` — the user interface (desktop and browser): `mod.rs` (state and frame loop), `project_file.rs` (live file, saves, undo), `solving.rs`, `shell.rs` (sidebar, toast), `camera.rs` (pan, zoom, viewport), `memo.rs`, `search.rs`, `theme/`, and one module per page: `chart/` (scene, canvas, card, toolbar, inspector, person, moves, preferences, edits), `graph/` (data, force layout, view, panel), `categories/` (spellings, merges, entries), `people/` (list, overview, editor, rule card, roster), `policy.rs`, `room.rs`, `printing.rs`.

## Local agent workflow

1. Read the project file (in the app's data folder, see the guide) and the output of `cargo run -- schema`.
2. Add or revise `rules` in the project file. Keep attendee IDs stable and retain verbatim source text in each rule's `source` field. Preserve unresolved source notes with `reviewed: false`.
3. Validate the edited file with `cargo run -- validate <file>`.
4. Replace the file atomically (write a temporary JSON file in the same directory, validate it, then rename it). The open desktop app reloads the new state automatically.

The app keeps showing the last valid project if an external edit is incomplete or invalid, and it will not overwrite that edit. Its own saves use a temporary file and rename.

Interpret each wish **from the requester's perspective**. A one-way wish creates one rule. Mutual wishes create two rules. `Target::People` contains every named person the requester wants nearby; a multi-person target means all of them. `Target::Tag` and `Target::Organization` can specify how many members should be nearby. Add exact group tags to attendees before using them in rules. Do not guess identities from ambiguous names or conditional remarks.

Use `weight_mode: "ShareRequesterBudget"` for ordinary wishes. The app divides their weight across that requester's enabled shared-budget rules, so five individual wishes have no greater combined budget than one wish. Use `"Fixed"` for stronger individual requirements such as an avec. The default distance penalties are:

| Kind | Beside | Opposite | Diagonal | Near (≤ 2 rows) | Same table | Far |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Avec | 0 | 3 | 18 | 22 | 25 | 40 |
| Ordinary wish | 0 | 0 | 2 | 8 | 15 | 40 |

These are editable per rule. A proximity rule can also set `max_class` to make a distance a hard limit. For placement rules, `must_be: false` expresses exclusions; `hard` determines whether violating it is forbidden by the solver. `Area::Tag` can express accessibility or aisle requirements once the relevant seats are tagged.

Attendees with no enabled constraints are free attendees. The solver optimizes the people referenced by rules, then shuffles free attendees across the remaining seats. When generating another option, it keeps a free attendee's previous seat if that seat remains available.
