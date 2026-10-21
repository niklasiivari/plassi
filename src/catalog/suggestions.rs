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
    squash(a) == squash(b) || abbreviates(a, b) || abbreviates(b, a)
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
            .map(|(s, _)| (s.clone(), s.clone())),
    );
    list
}

/// "ABC/XYZ", "ABC & XYZ", "ABC, XYZ" → parts, if there are several.
fn parts(spelling: &str) -> Vec<String> {
    let parts: Vec<String> = spelling
        .split(['/', ',', '&', '+', ';'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    if parts.len() > 1 { parts } else { Vec::new() }
}

/// Suggested mappings for every unmapped spelling of a kind.
pub fn merge_suggestions(project: &Project, kind: Kind) -> Vec<Merge> {
    let spellings = project.spellings(kind);
    let all_known = candidates(project, kind, &spellings, "");
    let mut result = Vec::new();
    for (spelling, _) in &spellings {
        if project.catalog.find(kind, spelling).is_some() {
            continue;
        }
        // Several things in one spelling: every part must be known.
        let split: Option<Vec<String>> = parts(spelling)
            .iter()
            .map(|part| {
                all_known
                    .iter()
                    .find(|(known, _)| same_thing(part, known))
                    .map(|(_, name)| name.clone())
            })
            .collect();
        if let Some(targets) = split.filter(|t| !t.is_empty()) {
            result.push(Merge {
                spelling: spelling.clone(),
                targets,
            });
            continue;
        }
        let known = candidates(project, kind, &spellings, spelling);
        if let Some((_, name)) = known.iter().find(|(other, _)| same_thing(spelling, other)) {
            result.push(Merge {
                spelling: spelling.clone(),
                targets: vec![name.clone()],
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
