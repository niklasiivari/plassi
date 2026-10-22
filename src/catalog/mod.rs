//! Categories for organizations, ticket types, food and drinks.
//!
//! People type these freely in sign-up forms ("ABC", "Abc", "Aa-Bb Cc").
//! Attendees keep what they wrote; the catalog maps each spelling to one
//! category, which everything else (the solver, wishes, prints) uses.

use crate::i18n::tr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Attendee;

mod membership;
mod suggestions;

pub use suggestions::{Merge, merge_suggestions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Organization,
    Ticket,
    Food,
    Drink,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Organization, Kind::Ticket, Kind::Food, Kind::Drink];

    pub fn plural(self) -> &'static str {
        match self {
            Kind::Organization => tr("Organizations"),
            Kind::Ticket => tr("Ticket types"),
            Kind::Food => tr("Food"),
            Kind::Drink => tr("Drinks"),
        }
    }

    /// A stable name for settings, e.g. "food".
    pub fn key(self) -> &'static str {
        match self {
            Kind::Organization => "organization",
            Kind::Ticket => "ticket",
            Kind::Food => "food",
            Kind::Drink => "drink",
        }
    }

    pub fn from_key(key: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// What the attendee wrote for this kind.
    pub fn raw(self, attendee: &Attendee) -> &str {
        match self {
            Kind::Organization => &attendee.organization,
            Kind::Ticket => &attendee.participant_group,
            Kind::Food => &attendee.food,
            Kind::Drink => &attendee.drink,
        }
    }
}

/// One canonical value, e.g. the organization "ABC".
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Category {
    pub name: String,
    /// Spellings that mean this category (compared case- and space-insensitively).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Additive charge in euro cents; None means the price has not been set.
    #[serde(default)]
    pub price_cents: Option<u32>,
    /// Short mark for the kitchen and bar prints, e.g. "VG". Empty prints the name.
    #[serde(default)]
    pub mark: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default)]
pub struct Catalog {
    /// Imported entries deliberately left without a category, by kind.
    pub ignored: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    pub organizations: Vec<Category>,
    pub tickets: Vec<Category>,
    pub foods: Vec<Category>,
    pub drinks: Vec<Category>,
}

/// Lower-case with single spaces, for comparing spellings.
/// Lowercase, single-spaced, without punctuation at either end: the form in
/// which names and spellings are compared.
pub fn normalize(text: &str) -> String {
    let trimmed = text.trim();
    let words = trimmed
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // A dash or other punctuation-only entry is still a distinct spelling.
    if words.is_empty() {
        trimmed.to_lowercase()
    } else {
        words
    }
}

/// Spellings to categories, as `Catalog::find_all` finds them.
pub struct Lookup(std::collections::HashMap<String, Vec<usize>>);

impl Lookup {
    pub fn find_all(&self, raw: &str) -> &[usize] {
        self.0.get(&normalize(raw)).map_or(&[], Vec::as_slice)
    }
}

impl Category {
    pub fn matches(&self, raw: &str) -> bool {
        let raw = normalize(raw);
        !raw.is_empty()
            && (normalize(&self.name) == raw || self.aliases.iter().any(|a| normalize(a) == raw))
    }
}

impl Catalog {
    pub fn is_ignored(&self, kind: Kind, raw: &str) -> bool {
        let key = normalize(raw);
        self.ignored
            .get(kind.key())
            .is_some_and(|entries| entries.contains(&key))
    }

    pub fn ignore(&mut self, kind: Kind, raw: &str, ignored: bool) {
        let key = normalize(raw);
        let entries = self.ignored.entry(kind.key().into()).or_default();
        entries.remove(&key);
        if ignored && !key.is_empty() {
            entries.insert(key);
        }
    }

    /// Category indices in display order, without changing stored memberships.
    pub fn sorted_indices(&self, kind: Kind) -> Vec<usize> {
        let mut indices: Vec<_> = (0..self.list(kind).len()).collect();
        indices.sort_by_cached_key(|&i| normalize(&self.list(kind)[i].name));
        indices
    }

    /// A category entered by the planner, even if its name was previously an
    /// alias of another category. Empty and duplicate names are rejected.
    pub fn create_named(&mut self, kind: Kind, name: &str) -> Option<usize> {
        let name = name.trim();
        let key = normalize(name);
        if key.is_empty() || self.list(kind).iter().any(|c| normalize(&c.name) == key) {
            return None;
        }
        let list = self.list_mut(kind);
        list.push(Category {
            name: name.into(),
            ..Default::default()
        });
        Some(list.len() - 1)
    }

    /// Removes only this membership; other categories for a combined entry stay.
    pub fn unassign(&mut self, kind: Kind, raw: &str, target: usize) {
        let key = normalize(raw);
        self.list_mut(kind)[target]
            .aliases
            .retain(|alias| normalize(alias) != key);
    }

    pub fn list(&self, kind: Kind) -> &Vec<Category> {
        match kind {
            Kind::Organization => &self.organizations,
            Kind::Ticket => &self.tickets,
            Kind::Food => &self.foods,
            Kind::Drink => &self.drinks,
        }
    }

    pub fn list_mut(&mut self, kind: Kind) -> &mut Vec<Category> {
        match kind {
            Kind::Organization => &mut self.organizations,
            Kind::Ticket => &mut self.tickets,
            Kind::Food => &mut self.foods,
            Kind::Drink => &mut self.drinks,
        }
    }

    /// Index of the first category a spelling belongs to.
    pub fn find(&self, kind: Kind, raw: &str) -> Option<usize> {
        self.list(kind).iter().position(|c| c.matches(raw))
    }

    /// Every category a spelling belongs to; "ABC/XYZ" can mean both.
    pub fn find_all(&self, kind: Kind, raw: &str) -> Vec<usize> {
        let list = self.list(kind);
        (0..list.len()).filter(|&i| list[i].matches(raw)).collect()
    }

    /// The categories of every normalized name and alias, for looking up
    /// many spellings at once.
    pub fn lookup(&self, kind: Kind) -> Lookup {
        let mut map: std::collections::HashMap<String, Vec<usize>> = Default::default();
        for (index, category) in self.list(kind).iter().enumerate() {
            for spelling in std::iter::once(&category.name).chain(&category.aliases) {
                let key = normalize(spelling);
                if key.is_empty() {
                    continue;
                }
                let found = map.entry(key).or_default();
                if !found.contains(&index) {
                    found.push(index);
                }
            }
        }
        for found in map.values_mut() {
            found.sort_unstable();
        }
        Lookup(map)
    }

    /// Adds a spelling to a category (by index). It keeps any other categories.
    pub fn assign(&mut self, kind: Kind, raw: &str, target: usize) {
        self.ignore(kind, raw, false);
        let category = &mut self.list_mut(kind)[target];
        if !category.matches(raw) {
            category.aliases.push(raw.trim().to_string());
        }
    }

    /// Creates a category named after a spelling and maps the spelling to it.
    pub fn create(&mut self, kind: Kind, raw: &str) -> usize {
        self.ignore(kind, raw, false);
        if let Some(existing) = self.find(kind, raw) {
            return existing;
        }
        self.list_mut(kind).push(Category {
            name: raw.trim().to_string(),
            aliases: Vec::new(),
            price_cents: None,
            mark: String::new(),
        });
        self.list(kind).len() - 1
    }

    /// Removes a category; its spellings become unmapped.
    pub fn remove(&mut self, kind: Kind, index: usize) {
        let list = self.list_mut(kind);
        if index < list.len() {
            list.remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::suggestions::initials;
    use super::*;

    #[test]
    fn named_categories_need_no_attendee_and_memberships_are_independent() {
        let mut catalog = Catalog::default();
        assert_eq!(catalog.create_named(Kind::Organization, "  "), None);
        let a = catalog.create_named(Kind::Organization, "Org A").unwrap();
        catalog.assign(Kind::Organization, "Org A + Org B", a);
        // Even a name previously used as an alias can become its own organization.
        catalog.assign(Kind::Organization, "Org B", a);
        let b = catalog.create_named(Kind::Organization, " Org B ").unwrap();
        assert_eq!(catalog.create_named(Kind::Organization, "org b"), None);
        catalog.assign(Kind::Organization, "Org A + Org B", b);
        assert_eq!(
            catalog.find_all(Kind::Organization, "Org A + Org B"),
            [a, b]
        );
        catalog.unassign(Kind::Organization, "Org A + Org B", a);
        assert_eq!(catalog.find_all(Kind::Organization, "Org A + Org B"), [b]);
        assert_eq!(catalog.find(Kind::Organization, "Org A"), Some(a));
    }

    #[test]
    fn spellings_map_to_one_category() {
        let mut catalog = Catalog::default();
        let abc = catalog.create(Kind::Organization, "ABC");
        catalog.assign(Kind::Organization, "Aa-Bb  cc", abc);
        assert_eq!(catalog.find(Kind::Organization, "abc"), Some(abc));
        assert_eq!(catalog.find(Kind::Organization, "Aa-Bb Cc"), Some(abc));
        assert_eq!(catalog.find(Kind::Organization, "XYZ"), None);
        assert_eq!(initials("Aa-Bb Cc"), "abc");
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::*;
    #[test]
    fn punctuation_entries_can_be_mapped_ignored_reopened_and_persisted() {
        let mut catalog = Catalog::default();
        let none = catalog.create_named(Kind::Organization, "ABC").unwrap();
        catalog.assign(Kind::Organization, "-", none);
        assert_eq!(catalog.find_all(Kind::Organization, " - "), [none]);
        assert_eq!(catalog.lookup(Kind::Organization).find_all("-"), [none]);
        assert!(catalog.find(Kind::Organization, ".").is_none());
        catalog.ignore(Kind::Organization, "?", true);
        let mut catalog: Catalog =
            serde_json::from_str(&serde_json::to_string(&catalog).unwrap()).unwrap();
        assert!(catalog.is_ignored(Kind::Organization, " ? "));
        assert!(!catalog.is_ignored(Kind::Food, "?"));
        catalog.ignore(Kind::Organization, "?", false);
        assert!(!catalog.is_ignored(Kind::Organization, "?"));
        catalog.ignore(Kind::Organization, "?", true);
        catalog.assign(Kind::Organization, "?", none);
        assert!(!catalog.is_ignored(Kind::Organization, "?"));
        assert!(catalog.create_named(Kind::Organization, "-").is_some());
        assert_eq!(catalog.sorted_indices(Kind::Organization), [1, 0]);
    }
}
