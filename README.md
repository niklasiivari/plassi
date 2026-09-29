# Plassi

Local seating planner written in Rust. The project JSON is the source of truth for attendees, room geometry, structured constraints, and candidate charts. The desktop app reloads valid external JSON edits while it is open. No service or hosted backend is required.

The desktop build supports Wayland and X11 on Linux. Everything is pure Rust apart from system fonts; no external solver is needed.

## Run

```sh
cargo run -- gui
```

Every event is a project. The desktop app keeps projects as JSON files in its data folder (`~/.local/share/plassi/projects/` on Linux, `~/Library/Application Support/Plassi/projects/` on macOS, `%APPDATA%\Plassi\projects\` on Windows; `PLASSI_HOME` overrides it) and reopens the last one. The switcher at the top of the sidebar opens, creates, duplicates, renames and deletes projects; editing a project file from outside the app is picked up live. New here? "Try with example data" on the chart or People page opens a made-up event with 48 guests, wishes and a room.

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
- Printing is a separate module (`typeset/`, Typst plus the bundled Source Sans and Source Serif fonts) loaded the first time you print, and PDFs arrive as downloads. It is split off because Typst is most of the size: the app is about 5 MB and the typesetter about 24 MiB, each under Cloudflare Pages' 25 MiB file limit.

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
- `src/app/` — the desktop UI: `mod.rs` (state and frame loop), `project_file.rs` (live file, saves, undo), `solving.rs`, `shell.rs` (sidebar, toast), `camera.rs` (pan, zoom, viewport), `memo.rs`, `search.rs`, `theme/`, and one module per page: `chart/` (scene, canvas, card, toolbar, inspector, person, moves, preferences, edits), `graph/` (data, force layout, view, panel), `categories/` (spellings, merges, entries), `people/` (list, overview, editor, rule card, roster), `policy.rs`, `room.rs`, `printing.rs`.

## Local agent workflow

1. Read `data/data.tsv`, `plassi.json`, and the output of `cargo run -- schema`.
2. Add or revise `rules` in `plassi.json`. Keep attendee IDs stable and retain verbatim source text in each rule's `source` field. Preserve unresolved source notes with `reviewed: false`.
3. Validate the edited file with `cargo run -- validate plassi.json`.
4. Replace `plassi.json` atomically (write a temporary JSON file in the same directory, validate it, then rename it). The open desktop app reloads the new state automatically.

The app keeps showing the last valid project if an external edit is incomplete or invalid, and it will not overwrite that edit. Its own saves use a temporary file and rename.

Interpret each wish **from the requester's perspective**. A one-way wish creates one rule. Mutual wishes create two rules. `Target::People` accepts one or several person IDs; `required: 1` means at least one of them should be close, while a larger `required` asks for that many nearby people. `Target::Tag` works the same way for a group. Add exact group tags to attendees before using them in rules. Do not guess identities from ambiguous names or conditional remarks.

Use `weight_mode: "ShareRequesterBudget"` for ordinary wishes. The app divides their weight across that requester's enabled shared-budget rules, so five individual wishes have no greater combined budget than one wish. Use `"Fixed"` for stronger individual requirements such as an avec. The default distance penalties are:

| Kind | Beside | Opposite | Diagonal | Same table | Far |
| --- | ---: | ---: | ---: | ---: | ---: |
| Avec | 0 | 3 | 18 | 25 | 40 |
| Ordinary wish | 0 | 0 | 2 | 15 | 40 |

These are editable per rule. A proximity rule can also set `max_class` to make a distance a hard limit. For placement rules, `must_be: false` expresses exclusions; `hard` determines whether violating it is forbidden by the solver. `Area::Tag` can express accessibility or aisle requirements once the relevant seats are tagged.

Attendees with no enabled constraints are free attendees. The solver optimizes the people referenced by rules, then shuffles free attendees across the remaining seats. When generating another option, it keeps a free attendee's previous seat if that seat remains available.

## Importing a spreadsheet

Plassi has a spreadsheet template. Keep your sign-ups in these columns (any order; English or Finnish headers; only the name is required):

| English | Suomi | What goes in it |
| --- | --- | --- |
| Name | Nimi | Full name; people are matched by it |
| Organization | Järjestö | Organization or club |
| Participant group | Osallistujaryhmä | Ticket type or participant group |
| Avec | Avec | The avec's full name, as written |
| Seating wish | Pöytäseuratoive | Who they want to sit with, in their own words |
| Food | Ruoka | Menu choice |
| Drinks | Juoma | Drink choice |
| Allergies | Allergiat | Allergies and diets |

On People & wishes, "Copy header row" puts the header on the clipboard for the first row of your sheet, "Download template" saves a .csv with the header and an example row, and "Export people" saves everyone in these columns to edit and import back.

To import, paste the rows (header row included) or drop a .csv/.tsv file. Before anything changes, a preview lists each column with its first values and what it will be imported as. Template headers (English or Finnish, any case) are recognised exactly. Nothing else is guessed: any other column is kept as extra info until you choose what it is from its menu (or "Ignore"). Your choices are remembered in the project, so importing the same sheet again maps those columns the same way. The command-line `import` only understands template headers. People are matched by name, so importing again updates them and keeps rules, the room and options. New sign-up text becomes unreviewed notes.

## Suggested wishes

The app reads unreviewed sign-up notes and suggests wishes: full names inside longer text (sure), misspelled names or a lone surname (likely), a lone unique first name (maybe), names joined with "tai"/"or" (one wish satisfied by either person), and organization names such as "XYZ" (a wish to sit with that organization's members, `Target::Organization`). Each suggestion has an Add button; the overview can add all full-name matches at once.

## Groups and hand editing

People connected by person-to-person wishes form a wish group. Selecting someone highlights their group; "Find places for this group" ranks blocks of seats where the whole group fits, with the people there swapping into the group's old seats. "Improve" re-optimizes the current option as a new option, keeping pinned people and everyone moved by hand (`hand_placed`).

## Your preferences

Ctrl- or Shift-click several people on the chart, then choose **Group together** or
**Keep apart** (not beside or opposite, or not at the same table). "Together" counts
as if each of them had wished for every other. Preferences are stored apart from
attendees' own wishes, under `preferences` in the project file. The side panel lists
them with how each one fares; there you can set the strength (nice to have,
important, must) or delete one. "Must" is never broken by the solver. Hover a
preference to outline its people on the chart.

## Categories

Organizations, ticket types, food and drinks are categories (`catalog` in the project file). Attendees keep what they wrote; each category lists the spellings that mean it, compared ignoring case and spacing. One spelling can belong to several categories, so "ABC/XYZ" counts as both ABC and XYZ. The Categories page lists every spelling with its count, suggests merges (same letters, an abbreviation like "Aa-Bb Cc" → "ABC", or several known things in one spelling), and lets you create, rename, merge and delete categories. The solver's organization mixing, organization wishes, avatar colours and print totals all use categories. Tags are only for your own labels, e.g. for "sit near everyone tagged X" wishes.

## Connections

The Connections page draws everyone with wishes as a force-directed graph: arrows point from the person who wished, mutual wishes are plain lines, avecs are thicker, and each line is coloured by whether that wish is met in the current option. "Arrange by table" takes the room's shape: tables drawn tall and side by side as on the chart, each person pulled towards their own seat, so wishes that cross between tables (split groups) stand out. Each table says how many of its wishes reach other tables. Drag people to rearrange; click to select (the selection is shared with the chart and People pages).

## Table ends

"Not at a table end" (People & wishes → + Add) is a per-person rule: a larger penalty for an end seat (nobody beside them on one side) and a smaller one for the seat next to it. Ends follow who actually sits where, so spare seats and removals move them.

## Late changes

On People & wishes: "+ Add person" adds a walk-in and seats them in the best free seat of every option (never leaving a gap); "Replace…" puts a new person in someone's seat in every option; "Remove" handles a cancellation, freeing their seats and removing their wishes and wishes about them. Adding needs a free seat. After changes, Improve or Close gaps on the chart tidy the seating. The logic is in `src/roster.rs`.

## Language

The interface is in English or Finnish; the EN/FI button next to the light/dark toggle switches, and the choice is remembered (the first launch follows the system or browser language). Text is written in English in the code and passed through `i18n::tr` / `i18n::fill`; Finnish lives in `src/i18n/fi/`, one catalog per part of the app. Missing entries fall back to English. The prints have their own language setting on the Print page.

## Keys

On the chart: Ctrl+F finds someone (arrow keys and Enter pick from the matches), F fits the room, P pins or unpins the selected person, G groups the Ctrl-clicked people, Esc clears. Ctrl+Z / Ctrl+Shift+Z undo and redo everywhere; the status message after an edit has its own Undo button.

## Printing

"Save all four PDFs" writes every print at once. The Print page checks the chosen option first (hard rules broken, people without a seat, empty seats mid-table) and marks saved PDFs whose project has changed since.

The Print page saves A4 PDFs next to the project file, typeset with a Typst template (`typeset/src/print.typ`) and the bundled Source Serif and Source Sans fonts:

- Seating chart: all tables with names for the wall, plus an alphabetical list of seats.
- Kitchen list: food and allergies per table and seat, with totals and an allergy summary.
- Bar list: drinks per table and seat, with totals.
- Door list: alphabetical ticket check with organization, price category, table and a tick box.

Event title, subtitle and print language (Finnish or English) are in `event`.

## Pins and seating policy

A pin is a hard `Placement` rule with `Area::Seat` (label `Pinned`). Pin or unpin someone from the chart (right-click a card, or the person panel). The solver never moves pinned people; dragging a pinned person moves their pin with them.

`policy` in the project JSON holds global rules, edited on the Seating policy page:

- `organization` (`SameOrganization`, `Mixed`, `Indifferent`) and `organization_weight`: cost per beside/opposite pair involving an unconstrained attendee that goes against the setting.
- `sprinkle`: positive values spread unconstrained attendees among wish groups; negative values keep them apart.
- `isolation_penalty`: cost for anyone with nobody beside or opposite them.
- `empty_seats`: `TableBottoms` leaves spare seats at the bottom of tables in left/right pairs (an odd one is the bottom right seat); `Anywhere` does not restrict them.

The seating policy is part of the same score the solver minimises (see Solving).

## Room geometry

Rows and tables are data in `layout.rows`. Tables are vertical: tables in a row sit side by side, and each table has independent `left` and `right` seat counts numbered from the top. Table ends are never generated as seating positions. Seat IDs are generated as `table-id:left:0`, `table-id:right:0`.

The template infers beside, opposite, diagonal and near (same table, at most two rows apart) positions for its standard seats; anything farther on the same table is "same table". `layout.relations` can override any pair, including pairs on different tables. The Room page exposes seat counts, tags, and relation overrides; an agent may also edit the JSON.

## Solving and review

The solver (`src/seating/`) is a tabu search, the approach used in the seating-plan and quadratic-assignment literature (Lewis & Carroll 2016; Taillard's robust tabu search; breakout local search). It minimises exactly the score the app shows: wish penalties, placement penalties and the seating policy. Each iteration evaluates every seat swap incrementally, takes the best one that is not tabu, and forbids moving people straight back for a while; when progress stalls it restarts from the best seating with a growing number of random swaps. The starting seating places pinned and restricted people first, then wish groups in contiguous blocks. Hard rules are never broken by the search. A wish for several people ("near 2 of my team") costs the average of its `required` closest members, so each one who comes closer helps; hard limits apply to the `required`-th closest. Spare seats go at table bottoms, but never where people limited by hard rules would run out of room.

A solve runs one independent search per CPU core for 30 seconds (Improve: 10 seconds): quality matters more than waiting. The status says "Optimal" only when every wish is at its cheapest distance and nothing can score lower; otherwise it reports how many runs reached the same score. The score shown in the app is the solver's own score, part by part. "Different option" requires roughly 10% of constrained attendees to move. `plassi bench PROJECT.json [SECONDS] [RUNS]` runs the search with fixed seeds for tuning (`PLASSI_TRIALS` sets how many times; default 5).

The Chart page shows the option's score (wish penalties plus seating-policy cost; lower is better) and every broken wish. Hand editing:

- Drag a person onto another seat to swap them. While dragging, a badge shows how the score would change and whether a hard rule would break.
- Right-click a person to pin them, or choose "Swap with…" and click the second person.
- The person panel lists "Better seats": the swaps that improve the score most, each with an Apply button.
- Ctrl+Z / Ctrl+Shift+Z undo and redo any saved change. Right-click an option chip to duplicate it (edit a copy), compare it with the current one, or delete it.
- Moves that would leave an empty seat mid-table are refused. Options with such gaps are flagged, and "Close gaps" slides people up each table side (pinned people stay put).

Manual edits may violate hard rules; the report shows them rather than silently moving people back.

`plassi.json` is ignored by Git because it contains attendee names.

## Backups

Earlier versions of each project are kept automatically: one every few minutes while editing, and one right before the project is deleted, replaced by an import or a loaded file, or overwritten by an external edit. On desktop they are in `backups/<project>/` in the data folder (the newest 20, plus one per day for two weeks); in the browser only the newest three per project are kept, and older backups make room if storage is full. The project menu's "Earlier versions…" lists them with Restore, which is an ordinary change that Undo can take back. "Deleted projects…" brings back a deleted project from its latest backup.
