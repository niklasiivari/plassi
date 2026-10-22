//! Suggested mappings for spellings nobody has mapped yet: the same thing
//! written differently, an abbreviation, or several things in one ("ABC/XYZ").

use super::Kind;
use crate::model::Project;

/// Map `spelling` to the categories named in `targets` (created if missing).
#[derive(Clone, Debug, PartialEq)]
pub struct Merge {
    pub spelling: String,
    pub targets: Vec<String>,
    pub reasons: Vec<MatchReason>,
    /// Per component, competing canonical matches requiring a choice.
    pub alternatives: Vec<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MatchReason {
    Exact,
    Alias,
    Spelling,
    Abbreviation,
    NewCategory,
}
impl MatchReason {
    pub fn label(self) -> &'static str {
        use crate::i18n::tr;
        match self {
            Self::Exact => tr("Exact name match"),
            Self::Alias => tr("Known alias"),
            Self::Spelling => tr("Same spelling, different punctuation"),
            Self::Abbreviation => tr("Abbreviation match"),
            Self::NewCategory => tr("New part of a combined entry"),
        }
    }
}

/// Letters and digits only, lower-case.
fn squash(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Initials of a multi-word name, splitting on spaces and hyphens:
/// "Aa-Bb Cc" → "abc".
#[cfg(test)]
pub(super) fn initials(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c == '-')
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_lowercase)
        .collect()
}

/// `short` abbreviates the multi-word `long`: every word contributes its
/// first letter, in order, and may add more letters from inside itself.
/// "Aa Bb" → AB, "Aa-Bb Cc" → ABC, "Xaqa Zz" → XQZ.
fn abbreviates(short: &str, long: &str) -> bool {
    let short: Vec<char> = squash(short).chars().collect();
    let words: Vec<Vec<char>> = long
        .split(|c: char| c.is_whitespace() || c == '-')
        .map(|w| squash(w).chars().collect::<Vec<_>>())
        .filter(|w| !w.is_empty())
        .collect();
    short.len() >= 2 && words.len() >= 2 && short.len() <= 2 * words.len() && covers(&short, &words)
}

/// Whether `letters` can be split so each word, in order, supplies its first
/// letter followed by zero or more of its later letters (in order).
fn covers(letters: &[char], words: &[Vec<char>]) -> bool {
    let Some((word, rest)) = words.split_first() else {
        return letters.is_empty();
    };
    if letters.first() != word.first() {
        return false;
    }
    // Take the first letter, then try every way of taking more letters from this word.
    (1..=letters.len()).any(|taken| {
        is_subsequence(&letters[1..taken], &word[1..]) && covers(&letters[taken..], rest)
    })
}

fn is_subsequence(needle: &[char], haystack: &[char]) -> bool {
    let mut rest = haystack.iter();
    needle.iter().all(|c| rest.any(|h| h == c))
}

fn same_thing(a: &str, b: &str) -> bool {
    let a_letters = squash(a);
    !a_letters.is_empty() && (a_letters == squash(b) || abbreviates(a, b) || abbreviates(b, a))
}

/// Every known way of writing something, with the name it should map to:
/// category names and aliases, then unmapped spellings more common than
/// `before` (so a pair of spellings isn't suggested both ways).
fn candidates(
    project: &Project,
    kind: Kind,
    spellings: &[(String, usize)],
    before: &str,
) -> Vec<(String, String)> {
    let mut list: Vec<(String, String)> = project
        .catalog
        .list(kind)
        .iter()
        .flat_map(|c| {
            std::iter::once(&c.name)
                .chain(&c.aliases)
                .map(move |s| (s.clone(), c.name.clone()))
        })
        .collect();
    list.extend(
        spellings
            .iter()
            .take_while(|(s, _)| s != before)
            .filter(|(s, _)| !project.catalog.is_ignored(kind, s))
            .map(|(s, _)| {
                let name = project
                    .catalog
                    .find(kind, s)
                    .map(|i| project.catalog.list(kind)[i].name.clone())
                    .unwrap_or_else(|| s.clone());
                (s.clone(), name)
            }),
    );
    list
}

/// "ABC/XYZ", "ABC & XYZ", "ABC, XYZ" → parts, if there are several.
fn parts(spelling: &str) -> Vec<String> {
    let spelling = spelling
        .split_whitespace()
        .map(|word| {
            if ["and", "ja", "sekä"].contains(&word.to_lowercase().as_str()) {
                "/"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let parts: Vec<String> = spelling
        .split(['/', ',', '&', '+', ';'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    if parts.len() > 1 { parts } else { Vec::new() }
}

fn matches(spelling: &str, known: &[(String, String)]) -> Vec<(String, MatchReason)> {
    let key = super::normalize(spelling);
    let squashed = squash(spelling);
    let mut found: Vec<_> = known
        .iter()
        .filter(|(s, _)| super::normalize(s) == key)
        .map(|(s, n)| {
            (
                n.clone(),
                if super::normalize(s) == super::normalize(n) {
                    MatchReason::Exact
                } else {
                    MatchReason::Alias
                },
            )
        })
        .collect();
    if found.is_empty() && !squashed.is_empty() {
        found = known
            .iter()
            .filter(|(s, _)| squash(s) == squashed)
            .map(|(_, n)| (n.clone(), MatchReason::Spelling))
            .collect();
    }
    if found.is_empty() {
        found = known
            .iter()
            .filter(|(s, _)| same_thing(spelling, s))
            .map(|(_, n)| (n.clone(), MatchReason::Abbreviation))
            .collect();
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup_by(|a, b| a.0 == b.0);
    found
}

fn proposal(spelling: &str, pieces: Vec<String>, known: &[(String, String)]) -> Merge {
    let mut merge = Merge {
        spelling: spelling.into(),
        targets: Vec::new(),
        reasons: Vec::new(),
        alternatives: Vec::new(),
    };
    for part in pieces {
        let found = matches(&part, known);
        let (name, reason) = found
            .first()
            .cloned()
            .unwrap_or((part, MatchReason::NewCategory));
        // Keep ambiguous components separate even when their defaults coincide.
        if found.len() <= 1 && merge.targets.contains(&name) {
            continue;
        }
        merge.targets.push(name);
        merge.reasons.push(reason);
        merge.alternatives.push(if found.len() > 1 {
            found.into_iter().map(|(n, _)| n).collect()
        } else {
            Vec::new()
        });
    }
    merge
}

/// Suggested mappings for every unmapped spelling of a kind.
pub fn merge_suggestions(project: &Project, kind: Kind) -> Vec<Merge> {
    let spellings = project.spellings(kind);
    let all_known = candidates(project, kind, &spellings, "");
    let mut result = Vec::new();
    for (spelling, _) in &spellings {
        if project.catalog.find(kind, spelling).is_some()
            || project.catalog.is_ignored(kind, spelling)
        {
            continue;
        }
        // Delimiters make each proposed membership explicit, including parts
        // that need a new category. Resolve exact spellings before abbreviations.
        let pieces = parts(spelling);
        if !pieces.is_empty() {
            let known: Vec<_> = all_known
                .iter()
                .filter(|(s, _)| s != spelling)
                .cloned()
                .collect();
            result.push(proposal(spelling, pieces, &known));
            continue;
        }
        let known = candidates(project, kind, &spellings, spelling);
        if !matches(spelling, &known).is_empty() {
            result.push(proposal(spelling, vec![spelling.clone()], &known));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::abbreviates;

    #[test]
    fn abbreviations() {
        assert!(abbreviates("AB", "Aa Bb"));
        assert!(abbreviates("ABC", "Aa Bb Cc"));
        assert!(abbreviates("ABC", "Aa-Bb Cc"));
        assert!(
            abbreviates("XQZ", "Xaqa Zz"),
            "extra letter from inside a word"
        );
        assert!(!abbreviates("ACB", "Aa Bb Cc"), "order matters");
        assert!(!abbreviates("AB", "Aa Bb Cc"), "every word contributes");
        assert!(!abbreviates("A", "Aa Bb"), "too short");
        assert!(!abbreviates("AB", "Aabb"), "needs several words");
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    use crate::catalog::Category;

    #[test]
    fn suggestions_keep_finding_new_and_existing_memberships_after_many_categories() {
        let mut project = crate::example::project();
        project.attendees.truncate(3);
        project.catalog.organizations = (0..200)
            .map(|i| Category {
                name: format!("ABC {i:03}"),
                aliases: vec![format!("XYZ {i:03}")],
                ..Default::default()
            })
            .collect();
        project.attendees[0].organization = "XYZ 199 / ABC 001".into();
        project.attendees[1].organization = "ABC 198, XYZ".into();
        project.attendees[2].organization = "ABC 197 and ABC 196".into();
        for _ in 0..3 {
            let suggestions = merge_suggestions(&project, Kind::Organization);
            assert_eq!(suggestions.len(), 3);
            assert_eq!(suggestions[0].targets, ["ABC 197", "ABC 196"]);
            assert!(
                suggestions
                    .iter()
                    .any(|m| m.targets == ["ABC 199", "ABC 001"])
            );
            assert!(suggestions.iter().any(|m| m.targets == ["ABC 198", "XYZ"]));
            project.catalog.create_named(
                Kind::Organization,
                &format!("XYZ {}", project.catalog.organizations.len()),
            );
        }
        project
            .catalog
            .ignore(Kind::Organization, "ABC 198, XYZ", true);
        assert_eq!(merge_suggestions(&project, Kind::Organization).len(), 2);
    }
}

#[cfg(test)]
mod explanation_tests {
    use super::*;
    #[test]
    fn ambiguity_is_retained_instead_of_picking_the_first_abbreviation() {
        let mut project = crate::example::project();
        project.attendees.truncate(1);
        project.attendees[0].organization = "AX".into();
        project.catalog.organizations.clear();
        for name in ["ABC XYZ", "ABC-XYZ"] {
            project.catalog.create_named(Kind::Organization, name);
        }
        let suggestions = merge_suggestions(&project, Kind::Organization);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].reasons, [MatchReason::Abbreviation]);
        assert_eq!(suggestions[0].alternatives[0], ["ABC XYZ", "ABC-XYZ"]);
        project.catalog.assign(Kind::Organization, "AX", 1);
        project.attendees[0].organization = "AX / XYZ".into();
        let suggestions = merge_suggestions(&project, Kind::Organization);
        assert!(suggestions[0].alternatives.iter().all(Vec::is_empty));
        assert_eq!(
            suggestions[0].reasons,
            [MatchReason::Alias, MatchReason::NewCategory]
        );
    }
}
