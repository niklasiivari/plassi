//! Suggested mappings for spellings nobody has mapped yet: the same thing
//! written differently, an abbreviation, or several things in one ("ABC/XYZ").

use super::Kind;
use crate::model::Project;

/// Map `spelling` to the categories named in `targets` (created if missing).
#[derive(Clone, Debug, PartialEq)]
pub struct Merge {
    pub spelling: String,
    pub targets: Vec<String>,
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

fn best_match(spelling: &str, known: &[(String, String)]) -> Option<String> {
    let key = super::normalize(spelling);
    known
        .iter()
        .find(|(s, _)| super::normalize(s) == key)
        .or_else(|| {
            known
                .iter()
                .find(|(s, _)| !squash(spelling).is_empty() && squash(s) == squash(spelling))
        })
        .or_else(|| known.iter().find(|(s, _)| same_thing(spelling, s)))
        .map(|(_, name)| name.clone())
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
            let mut targets: Vec<_> = pieces
                .iter()
                .map(|part| best_match(part, &known).unwrap_or_else(|| part.clone()))
                .collect();
            let mut seen = std::collections::HashSet::new();
            targets.retain(|name| seen.insert(name.clone()));
            result.push(Merge {
                spelling: spelling.clone(),
                targets,
            });
            continue;
        }
        let known = candidates(project, kind, &spellings, spelling);
        if let Some(name) = best_match(spelling, &known) {
            result.push(Merge {
                spelling: spelling.clone(),
                targets: vec![name],
            });
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
