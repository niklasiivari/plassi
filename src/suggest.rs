//! Suggests wishes from free-text sign-up notes: names hidden in longer text,
//! slightly misspelled names, lone surnames, and organization names.

use crate::i18n::{fill, tr};
use std::collections::{BTreeSet, HashMap};

use crate::catalog::Kind;
use crate::model::{Project, RuleKind, Target};

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
    pub wish: crate::model::WishKind,
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
    let length = word.chars().count();
    if length < 4 {
        word == token
    } else {
        // One edit changes the length by at most one.
        length.abs_diff(token.chars().count()) <= 1 && edit_distance(word, token) <= 1
    }
}

/// Suggestions for every unreviewed note, excluding wishes that already exist.
pub fn suggestions(project: &Project) -> Vec<Suggestion> {
    Suggester::default().suggestions(project)
}

pub fn for_note(project: &Project, note_index: usize) -> Vec<Suggestion> {
    let Some(note) = project.notes.get(note_index) else {
        return Vec::new();
    };
    let found = Index::new(project).found(note.person, &note.text);
    suggest(project, note_index, &found)
}

/// Keeps what each note names while the attendees and categories stay the
/// same, so reviewing a note or adding a wish only checks the wishes again.
#[derive(Default)]
pub struct Suggester {
    roster: u64,
    found: HashMap<(usize, String), Found>,
}

impl Suggester {
    /// Suggestions for every unreviewed note, excluding wishes that already exist.
    pub fn suggestions(&mut self, project: &Project) -> Vec<Suggestion> {
        let roster = roster_key(project);
        if roster != self.roster {
            self.found.clear();
            self.roster = roster;
        }
        let mut index = None;
        let mut result = Vec::new();
        for (i, note) in project.notes.iter().enumerate() {
            if note.reviewed {
                continue;
            }
            let found = self
                .found
                .entry((note.person, note.text.clone()))
                .or_insert_with(|| {
                    index
                        .get_or_insert_with(|| Index::new(project))
                        .found(note.person, &note.text)
                });
            result.extend(suggest(project, i, found));
        }
        result
    }
}

/// A fingerprint of what name and organization matches depend on.
fn roster_key(project: &Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for a in &project.attendees {
        (a.id, &a.name, &a.organization).hash(&mut hasher);
    }
    serde_json::to_string(&project.catalog)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

/// The people and organizations a note names, before leaving out wishes
/// that already exist.
#[derive(Clone)]
struct Found {
    people: Vec<(usize, Confidence, String)>,
    organizations: Vec<(String, Confidence, String)>,
}

/// A note's matches as suggestions, without the wishes its writer already has.
fn suggest(project: &Project, note_index: usize, found: &Found) -> Vec<Suggestion> {
    let note = &project.notes[note_index];
    let requester = note.person;
    let existing = wished_people(project, requester);
    let suggestion = |target, confidence, reason| Suggestion {
        note: note_index,
        requester,
        wish: note.field.wish(),
        target,
        confidence,
        reason,
    };
    let mut result: Vec<Suggestion> = found
        .people
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
    let kind = Kind::Organization;
    let has_rule = |name: &str| {
        let key = project.category_key_of(kind, name);
        project.rules.iter().any(|r| {
            matches!(&r.kind, RuleKind::Proximity { requester: q, target: Target::Organization { name: n, .. }, .. }
                if *q == requester && project.category_key_of(kind, n) == key)
        })
    };
    for (org, confidence, reason) in &found.organizations {
        if has_rule(org) {
            continue;
        }
        result.push(Suggestion {
            wish: crate::model::WishKind::Seating,
            ..suggestion(
                SuggestedTarget::Organization(org.clone()),
                *confidence,
                reason.clone(),
            )
        });
    }
    result
}

/// What matching needs from the project, prepared once for many notes.
struct Index<'a> {
    project: &'a Project,
    counts: NameCounts,
    names: Vec<(usize, Name)>,
    /// Folded, padded organization spellings with the category they mean.
    organizations: Vec<(String, String)>,
    /// Attendees per organization category key.
    members: HashMap<String, usize>,
}

/// An attendee's name prepared for matching.
struct Name {
    tokens: Vec<String>,
    /// " first last ", for whole-word `contains`.
    full: String,
    /// " last first ".
    reversed: String,
}

impl Name {
    fn new(name: &str) -> Option<Self> {
        let tokens = tokens(name);
        let (first, last) = (tokens.first()?, tokens.last()?);
        Some(Self {
            full: format!(" {} ", tokens.join(" ")),
            reversed: format!(" {last} {first} "),
            tokens,
        })
    }
}

impl<'a> Index<'a> {
    fn new(project: &'a Project) -> Self {
        let names = project
            .attendees
            .iter()
            .filter_map(|a| Some((a.id, Name::new(&a.name)?)))
            .collect();
        let organizations = organization_spellings(project)
            .into_iter()
            .map(|(spelling, category)| (padded_words(&fold(&spelling)), category))
            .filter(|(needle, _)| !needle.trim().is_empty())
            .collect();
        let mut members = HashMap::new();
        for attendee in &project.attendees {
            for key in project.category_keys(Kind::Organization, attendee) {
                *members.entry(key).or_default() += 1;
            }
        }
        Self {
            project,
            counts: NameCounts::new(project),
            names,
            organizations,
            members,
        }
    }

    fn found(&self, requester: usize, text: &str) -> Found {
        let text = NoteText::new(text);
        Found {
            people: self.mentioned_people(requester, &text),
            organizations: self.mentioned_organizations(requester, &text),
        }
    }

    /// Attendees other than the requester that the note names, most confident first.
    fn mentioned_people(
        &self,
        requester: usize,
        text: &NoteText,
    ) -> Vec<(usize, Confidence, String)> {
        let mut found: Vec<_> = self
            .names
            .iter()
            .filter(|(id, _)| *id != requester)
            .filter_map(|(id, name)| {
                name_match(name, text, &self.counts)
                    .map(|(confidence, reason)| (*id, confidence, reason))
            })
            .collect();
        found.sort_by_key(|(_, c, _)| *c);
        found
    }

    /// Organizations named as whole words, the requester's own counting as less likely.
    fn mentioned_organizations(
        &self,
        requester: usize,
        text: &NoteText,
    ) -> Vec<(String, Confidence, String)> {
        let project = self.project;
        let kind = Kind::Organization;
        let mut found: Vec<(String, Confidence, String)> = Vec::new();
        for (needle, category) in &self.organizations {
            if !text.padded.contains(needle.as_str()) {
                continue;
            }
            if found.iter().any(|(c, _, _)| c == category) {
                continue;
            }
            let own = project
                .person(requester)
                .map(|a| project.category_keys(kind, a))
                .unwrap_or_default();
            let key = project.category_key_of(kind, category);
            let members = self.members.get(&key).copied().unwrap_or(0);
            let confidence = if own.contains(&key) {
                Confidence::Low
            } else {
                Confidence::Medium
            };
            found.push((
                category.clone(),
                confidence,
                fill("mentions the organization ({} attending)", &[&members]),
            ));
        }
        found
    }
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

/// How confidently the note mentions `name`, and why.
fn name_match(name: &Name, text: &NoteText, counts: &NameCounts) -> Option<(Confidence, String)> {
    let Name {
        tokens: name_tokens,
        full,
        reversed,
    } = name;
    let (first, last) = (name_tokens.first()?, name_tokens.last()?);
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
    if text.padded.contains(full.as_str()) {
        Some((Confidence::High, tr("full name in text").to_string()))
    // Surname first, as in lists: "last first".
    } else if name_tokens.len() > 1 && text.padded.contains(reversed.as_str()) {
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
            suggestion.wish,
            &source,
        ),
        SuggestedTarget::Organization(name) => {
            let mut rule = crate::import::wish_rule(
                id,
                suggestion.requester,
                Vec::new(),
                crate::model::WishKind::Seating,
                &source,
            );
            rule.label = "Organization wish".into();
            if let RuleKind::Proximity { target, .. } = &mut rule.kind {
                *target = Target::Organization { name: name.clone() };
            }
            rule
        }
    };
    project.rules.push(rule);
}
