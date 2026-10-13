//! Every text written straight into a `tr` or `fill` call has a Finnish entry.

use std::collections::HashSet;
use std::path::Path;

/// String literals right after `tr(` or `fill(` in a Rust source.
fn literals(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for call in ["tr(", "fill("] {
        for (at, _) in source.match_indices(call) {
            // A whole word: not `str(` or `refill(`.
            let before = source[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let rest = source[at + call.len()..].trim_start();
            if let Some(text) = rest.strip_prefix('"').and_then(string_body) {
                found.push(text);
            }
        }
    }
    found
}

/// The text of a string literal, from just after its opening quote.
fn string_body(rest: &str) -> Option<String> {
    let mut text = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(text),
            '\\' => match chars.next()? {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                // A line continuation skips the next line's indent.
                '\n' => chars = chars.as_str().trim_start().chars(),
                other => text.push(other),
            },
            c => text.push(c),
        }
    }
    None
}

/// Rust sources under `dir`, leaving out the catalogs themselves.
fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source folder").flatten() {
        let path = entry.path();
        if path.ends_with("i18n") {
            continue;
        }
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_text_in_the_code_is_in_the_finnish_catalog() {
    let catalog: HashSet<&str> = super::fi::catalog().map(|(en, _)| en).collect();
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut missing = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).expect("source file");
        for text in literals(&source) {
            if !catalog.contains(text.as_str()) {
                missing.push(format!("{}: {text:?}", file.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "not translated:\n{}",
        missing.join("\n")
    );
}

#[test]
fn literals_are_read_with_their_escapes() {
    let source = r#"tr("a \"b\""); fill(
        "{} c", &[]); str("x"); refill("y");"#;
    assert_eq!(literals(source), ["a \"b\"", "{} c"]);
}
