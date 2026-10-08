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

pub use suggestions::merge_suggestions;

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
    /// Short mark for the kitchen and bar prints, e.g. "VE". Empty prints the name.
    #[serde(default)]
    pub mark: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default)]
pub struct Catalog {
    pub organizations: Vec<Category>,
    pub tickets: Vec<Category>,
    pub foods: Vec<Category>,
    pub drinks: Vec<Category>,
}

/// Lower-case with single spaces, for comparing spellings.
/// Lowercase, single-spaced, without punctuation at either end: the form in
/// which names and spellings are compared.
pub fn normalize(text: &str) -> String {
    text.trim()
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Category {
    pub fn matches(&self, raw: &str) -> bool {
        let raw = normalize(raw);
        !raw.is_empty()
            && (normalize(&self.name) == raw || self.aliases.iter().any(|a| normalize(a) == raw))
    }
}

impl Catalog {
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

    /// Adds a spelling to a category (by index). It keeps any other categories.
    pub fn assign(&mut self, kind: Kind, raw: &str, target: usize) {
        let category = &mut self.list_mut(kind)[target];
        if !category.matches(raw) {
            category.aliases.push(raw.trim().to_string());
        }
    }

    /// Creates a category named after a spelling and maps the spelling to it.
    pub fn create(&mut self, kind: Kind, raw: &str) -> usize {
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
