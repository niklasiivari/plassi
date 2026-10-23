# Rules for working in this repository

## No identifying information in the repository — ever

Nothing that identifies a real event, person, organization or place may go
into anything git tracks: code, comments, doc comments, docs, tests, test
data, i18n strings, file names, example data, the schema, commit messages.
This also applies to text that is only meant as an example.

- People: bare placeholders `X`, `Y`, `Z` (or `Person 1`, `Guest 01` as the
  existing tests do). No real or made-up first names or surnames.
- Organizations: `ABC`, `XYZ`.
- Events, projects, venues: `<project>`, `<event>`.
- Real attendee data lives outside the repo (the app's data folder,
  `~/.local/share/plassi/`, and `data/`, which is gitignored). Never copy
  anything from it into the repo, not even in a comment or a commit message
  describing what was done ("on a real event" is fine; its name is not).
- Before every commit, check the staged diff and the commit message for names
  from the real data (event title, attendee names, organization names).

## English first

English is the main language of the code and the app. Comments, doc
comments, docs, examples, test data and commit messages are in English.
UI text is written in English in the code and translated in `src/i18n/fi/`.
Finnish appears only in the Finnish catalogs, in the print template's
`t(en, fi)` pairs, and in code that has to recognise Finnish input.

## Other standing rules

- No backwards compatibility: no migrations, serde aliases or legacy paths.
  Change the project format directly.
- Desktop release builds run on every push to `main` and publish GitHub Releases;
  they must not build other branches. The separate CI workflow runs by hand.
  The website deploys through Cloudflare's own builds from `main`. Push only when asked.
- Plain wording in the UI and prints: no cute or marketing copy.
- Keep UI copy concise. Omit descriptions and instructions that repeat a label
  or explain an obvious control. Explain non-obvious behavior, consequences
  and recovery when needed.
