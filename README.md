# Plassi

A seating planner for events. Import guests from a spreadsheet, add seating wishes, arrange tables, and export the seating and catering lists.

Runs on Windows and Linux, or [in the browser](https://plassi.niklasiivari.fi). The interface is available in English and Finnish.

- Import Excel, ODS, CSV or TSV files, or paste spreadsheet rows and map their columns.
- Find seating that follows guests' wishes, group preferences and placement rules.
- Adjust seats by dragging, pin guests, compare options, and undo changes.
- Arrange tables in rows or freely on a canvas, with alignment guides and group spacing tools.
- Export seating, kitchen, bar and door lists as PDFs, or a seating workbook as Excel.
- Review wishes with keyboard shortcuts and return to recent changes through the activity history.

Projects stay on your device. The desktop app saves files locally; the browser uses browser storage. Guest data is not sent to a server.

## Download

Get the desktop app from [GitHub Releases](https://github.com/niklasiivari/plassi/releases/latest).

- **Windows:** run `Plassi-windows-x86_64.exe`. The ZIP includes the license and dependency notices. No installer is required; the executable is unsigned.
- **Linux:** make `Plassi-linux-x86_64.AppImage` executable and run it. Requires x86_64 Linux with compatible glibc (Ubuntu 22.04 or newer) and OpenGL drivers. Without FUSE, run `APPIMAGE_EXTRACT_AND_RUN=1 ./Plassi-linux-x86_64.AppImage`.

## Documentation

- [User guide](docs/GUIDE.md)
- [Development](docs/DEVELOPMENT.md)
- [Contributing](CONTRIBUTING.md)
- [Security reports](SECURITY.md)

## License

Copyright © 2026 Niklas Iivari and contributors. Licensed under [AGPL-3.0-or-later](LICENSE), without warranty.

Dependencies and fonts retain their own licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Forks and self-hosting are welcome. Identify your version and operator clearly; the license does not grant the original author's endorsement.
