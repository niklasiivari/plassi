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

Cloudflare builds and serves the browser version straight from this
GitHub repository (Workers Builds), so no GitHub Actions minutes are used.
`wrangler.jsonc` tells Cloudflare to serve `dist/` as a static site, and
`scripts/cf-build.sh` installs Rust, wasm-bindgen (the version in
`Cargo.lock`) and binaryen on the build machine before running
`scripts/build-web.sh`.

One-time setup in the Cloudflare dashboard (Workers & Pages → Create →
Import a repository → `niklasiivari/plassi`):

- Project name: `plassi`
- Build command: `./scripts/cf-build.sh`
- Deploy command: `npx wrangler deploy`
- Enable preview builds: off, unless you want a build for every branch.

Every push to `main` then builds and deploys, unless automatic builds are
turned off in the project's build settings. Builds count against
Cloudflare's free build minutes and take several minutes, because the
whole app is compiled for WebAssembly.

The site is static: no server-side code, headers or cookies are needed.
Tag the released commit: `git tag v0.1.0 && git push --tags`.

## CI

The "CI" workflow runs tests (including the UI click-through tests) and
clippy for the desktop and browser builds, with warnings as errors. Like
the deploy it only runs when started by hand (Actions → CI → Run
workflow, or `gh workflow run ci.yml`), to save the private repository's
Actions minutes; run it locally with `cargo test` and `cargo clippy`.

## Keep in mind

- The source code is proprietary (see LICENSE). The published site
  contains compiled code and the third-party notices, but not the source.
- Nothing in the repository may contain real attendee data. `data/`,
  project files and generated PDFs are gitignored; keep examples to
  placeholder names (Guest 01, X Y, ABC).
