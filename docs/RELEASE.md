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

The "Deploy web" GitHub Actions workflow builds the browser version and
deploys it to Cloudflare Pages. It only runs when started by hand:
Actions → Deploy web → Run workflow, or `gh workflow run deploy-web.yml`.

One-time Cloudflare setup:

1. In the Cloudflare dashboard, create a Pages project for direct uploads
   (Workers & Pages → Create → Pages → Upload assets) named `plassi`, or
   let the first deploy create it. A different name goes in the
   repository variable `CLOUDFLARE_PAGES_PROJECT`.
2. Create an API token (My Profile → API Tokens → Create Token) with the
   permission Account → Cloudflare Pages → Edit.
3. Add repository secrets in GitHub (Settings → Secrets and variables →
   Actions): `CLOUDFLARE_API_TOKEN` (the token) and
   `CLOUDFLARE_ACCOUNT_ID` (shown on the Workers & Pages overview).
   With the CLI: `gh secret set CLOUDFLARE_API_TOKEN` and
   `gh secret set CLOUDFLARE_ACCOUNT_ID`.
4. Optionally attach your own domain to the Pages project.

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
