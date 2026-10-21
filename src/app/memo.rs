//! A value recomputed only when its key changes (usually the project revision).

pub(in crate::app) struct Memo<K, V>(Option<(K, V)>);

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Self(None)
    }
}

impl<K: PartialEq, V> Memo<K, V> {
    /// The value for `key`, computed if the stored one is for another key.
    pub(in crate::app) fn get(&mut self, key: K, compute: impl FnOnce() -> V) -> &V {
        if self.0.as_ref().is_none_or(|(k, _)| *k != key) {
            self.0 = Some((key, compute()));
        }
        &self.0.as_ref().expect("just set").1
    }

    /// The stored value, if it is for `key`.
    pub(in crate::app) fn peek(&self, key: &K) -> Option<&V> {
        self.0.as_ref().filter(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(in crate::app) fn set(&mut self, key: K, value: V) {
        self.0 = Some((key, value));
    }

    pub(in crate::app) fn clear(&mut self) {
        self.0 = None;
    }
}
