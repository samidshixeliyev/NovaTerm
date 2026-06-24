//! An ordered key→value record (insertion order preserved for stable views).

use crate::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Record {
    entries: Vec<(String, Value)>,
}

impl Record {
    #[must_use]
    pub fn new() -> Self {
        Record {
            entries: Vec::new(),
        }
    }

    /// Insert or replace a field, preserving first-insertion position.
    pub fn push(&mut self, key: impl Into<String>, value: Value) {
        let key = key.into();
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    #[must_use]
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl FromIterator<(String, Value)> for Record {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        let mut r = Record::new();
        for (k, v) in iter {
            r.push(k, v);
        }
        r
    }
}
