//! What an attendee's free text means through the catalog: names, keys and
//! membership, possibly in several categories at once.

use super::{Kind, normalize};
use crate::model::{Attendee, Project};

impl Project {
    /// Ticket charge plus every selected food and drink charge, in euro cents.
    /// Unmapped or unpriced selections leave the total unknown.
    pub fn attendee_price_cents(&self, attendee: &Attendee) -> Option<u64> {
        if let Some(cents) = attendee.price_override_cents {
            return Some(u64::from(cents));
        }
        let mut total = 0u64;
        let mut has_selection = false;
        for kind in [Kind::Ticket, Kind::Food, Kind::Drink] {
            let raw = kind.raw(attendee).trim();
            if raw.is_empty() {
                continue;
            }
            has_selection = true;
            let categories = self.catalog.find_all(kind, raw);
            if categories.is_empty() {
                return None;
            }
            for index in categories {
                total += u64::from(self.catalog.list(kind)[index].price_cents?);
            }
        }
        has_selection.then_some(total)
    }

    /// Category names for an attendee, joined with " / ", or what they wrote if unmapped.
    pub fn category_name(&self, kind: Kind, attendee: &Attendee) -> String {
        let names = self.category_names(kind, attendee);
        if names.is_empty() {
            kind.raw(attendee).trim().to_string()
        } else {
            names.join(" / ")
        }
    }

    /// Names of every category the attendee belongs to.
    pub fn category_names(&self, kind: Kind, attendee: &Attendee) -> Vec<String> {
        let list = self.catalog.list(kind);
        self.catalog
            .find_all(kind, kind.raw(attendee))
            .into_iter()
            .map(|i| list[i].name.clone())
            .collect()
    }

    /// Comparison keys of every category the attendee belongs to; an unmapped
    /// spelling is its own single key. Empty text has no keys.
    pub fn category_keys(&self, kind: Kind, attendee: &Attendee) -> Vec<String> {
        let raw = kind.raw(attendee);
        let found = self.catalog.find_all(kind, raw);
        if found.is_empty() {
            let key = normalize(raw);
            return if key.is_empty() {
                Vec::new()
            } else {
                vec![key]
            };
        }
        let list = self.catalog.list(kind);
        found
            .into_iter()
            .map(|i| category_key(&list[i].name))
            .collect()
    }

    /// The first key, e.g. for choosing an avatar colour.
    pub fn category_key(&self, kind: Kind, attendee: &Attendee) -> String {
        self.category_keys(kind, attendee)
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    /// Key for a name typed in a rule (e.g. an organization wish): its category if known.
    pub fn category_key_of(&self, kind: Kind, text: &str) -> String {
        match self.catalog.find(kind, text) {
            Some(i) => category_key(&self.catalog.list(kind)[i].name),
            None => normalize(text),
        }
    }

    /// Whether the attendee belongs to the category (or spelling) behind `key`.
    pub fn in_category(&self, kind: Kind, attendee: &Attendee, key: &str) -> bool {
        self.category_keys(kind, attendee).iter().any(|k| k == key)
    }

    /// "Organization · ticket type" as category names, skipping empty parts.
    pub fn affiliation(&self, attendee: &Attendee) -> String {
        [Kind::Organization, Kind::Ticket]
            .map(|kind| self.category_name(kind, attendee))
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Distinct spellings in use for a kind, with how many people wrote each.
    pub fn spellings(&self, kind: Kind) -> Vec<(String, usize)> {
        let mut counts: std::collections::BTreeMap<String, (String, usize)> = Default::default();
        for a in &self.attendees {
            let raw = kind.raw(a).trim();
            if raw.is_empty() {
                continue;
            }
            let entry = counts
                .entry(normalize(raw))
                .or_insert_with(|| (raw.to_string(), 0));
            entry.1 += 1;
        }
        let mut list: Vec<_> = counts.into_values().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        list
    }
}

fn category_key(name: &str) -> String {
    format!("#{}", normalize(name))
}
