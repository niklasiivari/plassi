# Releasing Plassi

A checklist for publishing the browser version and tagging a release.

## Before

1. `cargo test` and `cargo clippy --all-targets` pass; so does
   `cargo clippy --target wasm32-unknown-unknown`.
2. `./scripts/notices.sh`, then check `THIRD_PARTY_NOTICES.md` is
   updated and committed (dependencies change the list).
3. Version: bump `version` in `Cargo.toml` (and `typeset/Cargo.toml`),
   move the CHANGELOG's "unreleased" heading to the version and date.
4. `./scripts/build-web.sh`; both `.wasm` files must stay under
   25 MiB (Cloudflare Pages' file limit; the script prints sizes).
5. Try the build: `python3 -m http.server -d dist 8080`, open it in a
   desktop browser and run one full session with the example data:
   import, solve, drag, print all four PDFs, download the project, load
   it back. Check the About window and the privacy text.
6. Check the Finnish interface once (switch with the FI button).

## Publish

- Deploy the contents of `dist/` as a static site (for example Cloudflare
  Pages). No server-side code, headers or cookies are needed.
- Tag the commit: `git tag v0.1.0`.

## Keep in mind

- The source code is proprietary (see LICENSE). The published site
  contains compiled code and the third-party notices, but not the source.
- Nothing in the repository may contain real attendee data. `data/`,
  project files and generated PDFs are gitignored; keep examples to
  placeholder names (Guest 01, X Y, ABC).
