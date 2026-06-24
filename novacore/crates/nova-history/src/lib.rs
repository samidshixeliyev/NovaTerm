//! `nova-history` — structured command history.
//!
//! Unlike a flat text history file, each entry records the source, the working
//! directory, the result type, and timing — so history is itself queryable
//! structured data (and renders in the UI as a table/timeline).

#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub id: u64,
    pub session: u64,
    pub source: String,
    pub cwd: String,
    /// Result type name (e.g. `"table"`, `"error"`).
    pub result_type: String,
    pub success: bool,
    pub duration_ns: i64,
}

#[derive(Default)]
pub struct History {
    entries: Vec<HistoryEntry>,
    next_id: u64,
    capacity: Option<usize>,
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        History::default()
    }

    /// Cap the in-memory ring; older entries are dropped (and would spill to a
    /// store in a later phase).
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        History {
            capacity: Some(cap),
            ..Default::default()
        }
    }

    pub fn append(&mut self, mut entry: HistoryEntry) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        entry.id = id;
        self.entries.push(entry);
        if let Some(cap) = self.capacity {
            while self.entries.len() > cap {
                self.entries.remove(0);
            }
        }
        id
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Most-recent entries first.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Case-insensitive substring search over the source, most-recent first.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|e| e.source.to_lowercase().contains(&q))
            .collect()
    }

    /// Reverse-search the previous command (Ctrl-R style): the most recent
    /// entry whose source contains `query`.
    #[must_use]
    pub fn last_matching(&self, query: &str) -> Option<&HistoryEntry> {
        self.search(query).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(src: &str) -> HistoryEntry {
        HistoryEntry {
            id: 0,
            session: 1,
            source: src.into(),
            cwd: "/".into(),
            result_type: "table".into(),
            success: true,
            duration_ns: 0,
        }
    }

    #[test]
    fn append_assigns_ids_and_orders_recent() {
        let mut h = History::new();
        h.append(entry("ls"));
        h.append(entry("ps"));
        h.append(entry("git status"));
        assert_eq!(h.len(), 3);
        let recent: Vec<_> = h.recent(2).iter().map(|e| e.source.clone()).collect();
        assert_eq!(recent, vec!["git status", "ps"]);
    }

    #[test]
    fn search_and_reverse() {
        let mut h = History::new();
        h.append(entry("git status"));
        h.append(entry("git commit"));
        h.append(entry("ls"));
        assert_eq!(h.search("git").len(), 2);
        assert_eq!(h.last_matching("git").unwrap().source, "git commit");
    }

    #[test]
    fn capacity_drops_oldest() {
        let mut h = History::with_capacity(2);
        h.append(entry("a"));
        h.append(entry("b"));
        h.append(entry("c"));
        assert_eq!(h.len(), 2);
        assert_eq!(
            h.recent(2)
                .iter()
                .map(|e| e.source.clone())
                .collect::<Vec<_>>(),
            vec!["c", "b"]
        );
    }
}
