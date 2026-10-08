//! Suggests wishes from free-text sign-up notes: names hidden in longer text,
//! slightly misspelled names, lone surnames, and organization names.

use crate::i18n::{fill, tr};
use std::collections::{BTreeSet, HashMap};

use crate::catalog::Kind;
use crate::model::{ImportField, Project, RuleKind, Target};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::High => tr("Exact match"),
            Confidence::Medium => tr("Likely match"),
            Confidence::Low => tr("Possible match"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SuggestedTarget {
    /// People explicitly named in the note.
    People(Vec<usize>),
    Organization(String),
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub note: usize,
    pub requester: usize,
    pub avec: bool,
    pub target: SuggestedTarget,
    pub confidence: Confidence,
    pub reason: String,
}

fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect()
}

fn tokens(text: &str) -> Vec<String> {
    fold(text).split_whitespace().map(str::to_string).collect()
}

/// Splits a note into name-sized pieces at punctuation and joining words.
fn segments(text: &str) -> Vec<Vec<String>> {
    const JOINERS: &[&str] = &["ja", "tai", "and", "or", "sekä", "tms", "&", "+"];
    let mut result = Vec::new();
    for piece in text.split([',', '/', ';', '\n', '(', ')', '.', '!', '?', ':']) {
        let mut current = Vec::new();
        for word in tokens(piece) {
            if JOINERS.contains(&word.as_str()) {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            } else {
                current.push(word);
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
    }
    result
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j] + usize::from(ca != *cb))
                    .min(prev[j + 1] + 1)
                    .min(cur[j] + 1),
            );
        }
        prev = cur;
    }
    prev[b.len()]
}

fn close(word: &str, token: &str) -> bool {
    if word.chars().count() < 4 {
        word == token
    } else {
        edit_distance(word, token) <= 1
    }
}

/// Suggestions for every unreviewed note, excluding wishes that already exist.
pub fn suggestions(project: &Project) -> Vec<Suggestion> {
    let mut result = Vec::new();
    for (index, note) in project.notes.iter().enumerate() {
        if !note.reviewed {
            result.extend(for_note(project, index));
        }
    }
    result
}

pub fn for_note(project: &Project, note_index: usize) -> Vec<Suggestion> {
    let Some(note) = project.notes.get(note_index) else {
        return Vec::new();
    };
    let requester = note.person;
    let text = NoteText::new(&note.text);
    let found = mentioned_people(project, requester, &text);
    let existing = wished_people(project, requester);
    let suggestion = |target, confidence, reason| Suggestion {
        note: note_index,
        requester,
        avec: note.field == ImportField::Avec,
        target,
        confidence,
        reason,
    };
    let mut result: Vec<Suggestion> = found
        .iter()
        .filter(|(id, _, _)| !existing.contains(id))
        .map(|(id, confidence, reason)| {
            suggestion(
                SuggestedTarget::People(vec![*id]),
                *confidence,
                reason.clone(),
            )
        })
        .collect();
    for (org, confidence, reason) in mentioned_organizations(project, requester, &text) {
        result.push(Suggestion {
            avec: false,
            ..suggestion(SuggestedTarget::Organization(org), confidence, reason)
        });
    }
    result
}

/// A note's text prepared for matching.
struct NoteText {
    /// Folded words joined by single spaces, padded with a space at both ends.
    padded: String,
    segments: Vec<Vec<String>>,
}

impl NoteText {
    fn new(text: &str) -> Self {
        Self {
            padded: padded_words(&fold(text)),
            segments: segments(text),
        }
    }
}

/// `" a b c "` from any whitespace-separated words, for whole-word `contains`.
fn padded_words(text: &str) -> String {
    format!(
        " {} ",
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

/// How many attendees share each first and each last name.
struct NameCounts {
    first: HashMap<String, usize>,
    last: HashMap<String, usize>,
}

impl NameCounts {
    fn new(project: &Project) -> Self {
        let mut counts = Self {
            first: HashMap::new(),
            last: HashMap::new(),
        };
        for a in &project.attendees {
            let t = tokens(&a.name);
            if let (Some(first), Some(last)) = (t.first(), t.last()) {
                *counts.first.entry(first.clone()).or_default() += 1;
                *counts.last.entry(last.clone()).or_default() += 1;
            }
        }
        counts
    }
}

/// Attendees other than the requester that the note names, most confident first.
fn mentioned_people(
    project: &Project,
    requester: usize,
    text: &NoteText,
) -> Vec<(usize, Confidence, String)> {
    let counts = NameCounts::new(project);
    let mut found: Vec<_> = project
        .attendees
        .iter()
        .filter(|a| a.id != requester)
        .filter_map(|a| {
            name_match(&a.name, text, &counts)
                .map(|(confidence, reason)| (a.id, confidence, reason))
        })
        .collect();
    found.sort_by_key(|(_, c, _)| *c);
    found
}

/// How confidently the note mentions `name`, and why.
fn name_match(name: &str, text: &NoteText, counts: &NameCounts) -> Option<(Confidence, String)> {
    let name_tokens = tokens(name);
    let (first, last) = (name_tokens.first()?, name_tokens.last()?);
    let full = format!(" {} ", name_tokens.join(" "));
    // Surname first, as in lists: "last first".
    let reversed = format!(" {last} {first} ");
    let initial = first.chars().next();
    // A segment is "first last" (maybe misspelled), just "last" with a
    // matching initial, or just the first name (optionally with an initial).
    let fuzzy_pair = |seg: &Vec<String>| {
        seg.windows(2).any(|w| {
            (close(first, &w[0]) && close(last, &w[1]))
                || (close(last, &w[0]) && close(first, &w[1]))
        })
    };
    let is_initial = |t: &String| t.chars().next() == initial && t.chars().count() <= 2;
    let surname_only = |seg: &Vec<String>| match seg.as_slice() {
        [l] => l == last,
        [f, l] => (l == last && f.chars().next() == initial) || (f == last && is_initial(l)),
        _ => false,
    };
    let first_only = |seg: &Vec<String>| match seg.as_slice() {
        [f] => f == first,
        [f, i] => f == first && i.chars().count() == 1 && last.starts_with(i.as_str()),
        _ => false,
    };
    let segments = &text.segments;
    if text.padded.contains(&full) {
        Some((Confidence::High, tr("full name in text").to_string()))
    } else if name_tokens.len() > 1 && text.padded.contains(&reversed) {
        Some((Confidence::High, tr("full name, surname first").to_string()))
    } else if name_tokens.len() > 1 && segments.iter().any(fuzzy_pair) {
        Some((Confidence::Medium, tr("name, maybe misspelled").into()))
    } else if last.chars().count() >= 4
        && counts.last.get(last) == Some(&1)
        && segments.iter().any(surname_only)
    {
        Some((
            Confidence::Medium,
            tr("only person with this surname").into(),
        ))
    } else if first.chars().count() >= 3
        && counts.first.get(first) == Some(&1)
        && segments.iter().any(first_only)
    {
        Some((
            Confidence::Low,
            tr("only person with this first name").into(),
        ))
    } else {
        None
    }
}

/// People the requester already has a proximity wish for.
fn wished_people(project: &Project, requester: usize) -> BTreeSet<usize> {
    project
        .rules
        .iter()
        .filter_map(|r| match &r.kind {
            RuleKind::Proximity {
                requester: q,
                target: Target::People { ids, .. },
                ..
            } if *q == requester => Some(ids.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Every way an organization can be written, with the category it means:
/// catalog names and aliases, plus spellings not in the catalog yet.
fn organization_spellings(project: &Project) -> Vec<(String, String)> {
    let kind = Kind::Organization;
    let mut result: Vec<(String, String)> = project
        .catalog
        .list(kind)
        .iter()
        .flat_map(|c| {
            std::iter::once(&c.name)
                .chain(&c.aliases)
                .map(move |s| (s.clone(), c.name.clone()))
        })
        .collect();
    for (spelling, _) in project.spellings(kind) {
        if project.catalog.find(kind, &spelling).is_none() {
            result.push((spelling.clone(), spelling));
        }
    }
    result.retain(|(s, _)| s.chars().count() >= 2);
    result
}

/// Organizations named as whole words that the requester has no wish for yet.
fn mentioned_organizations(
    project: &Project,
    requester: usize,
    text: &NoteText,
) -> Vec<(String, Confidence, String)> {
    let kind = Kind::Organization;
    let own = project
        .person(requester)
        .map(|a| project.category_keys(kind, a))
        .unwrap_or_default();
    let has_rule = |name: &str| {
        let key = project.category_key_of(kind, name);
        project.rules.iter().any(|r| {
            matches!(&r.kind, RuleKind::Proximity { requester: q, target: Target::Organization { name: n, .. }, .. }
                if *q == requester && project.category_key_of(kind, n) == key)
        })
    };
    let mut found: Vec<(String, Confidence, String)> = Vec::new();
    for (spelling, category) in organization_spellings(project) {
        let needle = padded_words(&fold(&spelling));
        if needle.trim().is_empty() || !text.padded.contains(&needle) || has_rule(&category) {
            continue;
        }
        if found.iter().any(|(c, _, _)| *c == category) {
            continue;
        }
        let key = project.category_key_of(kind, &category);
        let members = project
            .attendees
            .iter()
            .filter(|a| project.in_category(kind, a, &key))
            .count();
        let confidence = if own.contains(&key) {
            Confidence::Low
        } else {
            Confidence::Medium
        };
        found.push((
            category,
            confidence,
            fill("mentions the organization ({} attending)", &[&members]),
        ));
    }
    found
}

pub fn accept(project: &mut Project, suggestion: &Suggestion) {
    let id = project.next_rule_id();
    let source = project
        .notes
        .get(suggestion.note)
        .map(|n| n.text.clone())
        .unwrap_or_default();
    let rule = match &suggestion.target {
        SuggestedTarget::People(ids) => crate::import::wish_rule(
            id,
            suggestion.requester,
            ids.clone(),
            suggestion.avec,
            &source,
        ),
        SuggestedTarget::Organization(name) => {
            let mut rule =
                crate::import::wish_rule(id, suggestion.requester, Vec::new(), false, &source);
            rule.label = "Organization wish".into();
            if let RuleKind::Proximity { target, .. } = &mut rule.kind {
                *target = Target::Organization { name: name.clone() };
            }
            rule
        }
    };
    project.rules.push(rule);
}
