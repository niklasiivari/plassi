# Plassi

Plassi plans seating for events with long tables: dinners, gala evenings, sitsit. You import the guest list from a spreadsheet, Plassi reads who wants to sit with whom, finds a seating that meets as many wishes as possible, and prints the lists you need on the night.

It runs as a desktop app (Linux, macOS, Windows) or in the browser, in English or Finnish.

## What it does

- **Import from a spreadsheet** using a simple template (name, organization, participant group, avec, seating wish, food, drinks, allergies), or map your own columns once.
- **Understands wishes** written in sign-up forms ("with X and Y", avecs) and suggests them for review.
- **Finds a good seating** with a local search that keeps avecs and groups together, follows your seating policy (mix organizations or keep them together, nobody sits alone, spare seats at the bottom of tables) and respects hard rules and pinned seats.
- **Lets you adjust by hand**: drag people between seats with an instant score preview, pin seats, group or separate people, compare options, undo everything.
- **Handles late changes**: walk-ins, cancellations and replacements, with suggestions for where they fit.
- **Prints A4 PDFs**: the seating chart, kitchen list (food and allergies per table), bar list and door list.

## Privacy

Plassi keeps your projects on your own device: as files in the app's data folder on desktop, or in the browser's storage for the web version. Nothing is sent to a server. There are no accounts, no tracking and no cookies. Guest lists often contain personal and health information (allergies); keeping it local is deliberate.

## Running it

Desktop, from source (needs a recent Rust toolchain):

```sh
cargo run --release -- gui
```

The browser version is a static site built with `scripts/build-web.sh`; see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Documentation

- [docs/GUIDE.md](docs/GUIDE.md): using Plassi, page by page.
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md): building, testing, the project file format and the code layout.
- [CHANGELOG.md](CHANGELOG.md): what changed in each release.

## Licence

Copyright © 2026 Niklas Iivari. All rights reserved; see [LICENSE](LICENSE).

Plassi includes open-source software and fonts under their own licences; their notices are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and in the app under About.
