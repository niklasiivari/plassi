//! Case-insensitive "contains" matching for search boxes.

pub(in crate::app) struct Needle(String);

impl Needle {
    pub(in crate::app) fn new(query: &str) -> Self {
        Self(query.trim().to_lowercase())
    }

    pub(in crate::app) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether `text` contains the query; an empty query matches everything.
    pub(in crate::app) fn matches(&self, text: &str) -> bool {
        self.0.is_empty() || text.to_lowercase().contains(&self.0)
    }
}
