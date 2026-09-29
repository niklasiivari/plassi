//! The interface language. Text is written in English in the code and passed
//! through [`tr`]; the Finnish catalog in [`fi`] maps it, and anything missing
//! stays English. Values go into `{}` placeholders with [`fill`].
//!
//! The language is one setting for the whole app, so it is kept globally.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

mod fi;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Fi,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Fi];

    /// Short code, as stored in settings and shown on the switcher.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fi => "fi",
        }
    }

    /// The language's own name.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fi => "Suomi",
        }
    }

    /// A language from a code or locale such as "fi", "fi_FI.UTF-8" or "en-GB".
    pub fn from_locale(locale: &str) -> Option<Self> {
        let code = locale.get(..2)?.to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| l.code() == code)
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Fi,
        _ => Lang::En,
    }
}

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

fn finnish() -> &'static HashMap<&'static str, &'static str> {
    static CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    CATALOG.get_or_init(|| fi::catalog().collect())
}

/// `text` in the current language.
pub fn tr(text: &'static str) -> &'static str {
    match current() {
        Lang::En => text,
        Lang::Fi => finnish().get(text).copied().unwrap_or(text),
    }
}

/// Text that isn't known at compile time (e.g. a label saved in the project,
/// like "Avec"), translated when the catalog has it.
pub fn tr_any(text: &str) -> &str {
    match current() {
        Lang::En => text,
        Lang::Fi => finnish().get(text).copied().unwrap_or(text),
    }
}

/// `template` translated, with each `{}` replaced by the next value.
pub fn fill(template: &'static str, values: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut values = values.iter();
    let mut parts = tr(template).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for part in parts {
        if let Some(value) = values.next() {
            out.push_str(&value.to_string());
        }
        out.push_str(part);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_keeps_placeholders_and_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for (en, fi) in fi::catalog() {
            assert!(seen.insert(en), "duplicate entry: {en}");
            assert_eq!(en.matches("{}").count(), fi.matches("{}").count(), "placeholders differ: {en}");
            assert!(!fi.trim().is_empty(), "empty translation: {en}");
        }
    }

    #[test]
    fn fill_translates_and_substitutes() {
        set(Lang::En);
        assert_eq!(fill("{} of {} met", &[&3, &4]), "3 of 4 met");
    }
}
