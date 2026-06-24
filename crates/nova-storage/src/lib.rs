//! `nova-storage` — durable state in SQLite (WAL mode).
//!
//! Holds command history (with FTS search), workspaces and saved layouts,
//! sessions, snapshots/replay frames, themes, bookmarks, and analytics. The
//! schema and migrations live in [`migrations`]. The DB is opened lazily so it
//! never blocks startup.

#![forbid(unsafe_code)]

mod migrations;

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// A recorded command with timing and exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub command: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
}

/// The storage handle. Cheap to clone-by-reopen; typically one per app.
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Open (creating if needed) the database at `path`, applying migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Open an in-memory database (tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut storage = Storage { conn };
        migrations::run(&mut storage.conn)?;
        Ok(storage)
    }

    /// Current schema version.
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }

    // --- Sessions --------------------------------------------------------

    /// Insert or update a session row (so history/snapshots can reference it).
    pub fn upsert_session(
        &self,
        id: &str,
        profile_id: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        let ts = now();
        self.conn.execute(
            "INSERT INTO sessions (id, profile_id, cols, rows, state, created_at, last_active)
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET profile_id=?2, cols=?3, rows=?4, last_active=?5",
            params![id, profile_id, cols, rows, ts],
        )?;
        Ok(())
    }

    // --- Command history -------------------------------------------------

    pub fn record_command(&self, session_id: &str, entry: &HistoryEntry) -> Result<i64> {
        let ts = now();
        self.conn.execute(
            "INSERT INTO command_history (session_id, command, cwd, exit_code, duration_ms, ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                entry.command,
                entry.cwd,
                entry.exit_code,
                entry.duration_ms,
                ts
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        // Keep the FTS index in sync.
        self.conn.execute(
            "INSERT INTO command_history_fts (rowid, command) VALUES (?1, ?2)",
            params![id, entry.command],
        )?;
        Ok(id)
    }

    /// Full-text search over command history, most-recent first.
    pub fn search_history(&self, query: &str, limit: u32) -> Result<Vec<String>> {
        if query.trim().is_empty() {
            let mut stmt = self.conn.prepare(
                "SELECT command FROM command_history ORDER BY ts DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            return Ok(rows);
        }
        let mut stmt = self.conn.prepare(
            "SELECT h.command FROM command_history_fts f
             JOIN command_history h ON h.id = f.rowid
             WHERE command_history_fts MATCH ?1
             ORDER BY h.ts DESC, h.id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![fts_query(query), limit], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- Workspaces ------------------------------------------------------

    pub fn save_workspace(&self, id: &str, name: &str, layout_json: &str) -> Result<()> {
        let ts = now();
        self.conn.execute(
            "INSERT INTO workspaces (id, name, layout, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET name=?2, layout=?3, updated_at=?4",
            params![id, name, layout_json, ts],
        )?;
        Ok(())
    }

    pub fn load_workspace(&self, id: &str) -> Result<Option<(String, String)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, layout FROM workspaces WHERE id = ?1",
                params![id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?)
    }

    pub fn list_workspaces(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM workspaces ORDER BY updated_at DESC")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- Key/value (settings sync cursor, update channel, flags) ---------

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv (k, v, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(k) DO UPDATE SET v=?2, updated_at=?3",
            params![key, value, now()],
        )?;
        Ok(())
    }

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT v FROM kv WHERE k = ?1", params![key], |r| r.get(0))
            .optional()?)
    }

    // --- Themes ----------------------------------------------------------

    /// Insert a built-in theme if not already present (idempotent seeding).
    pub fn seed_theme(&self, id: &str, name: &str, palette_json: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO themes (id, name, builtin, palette, updated_at)
             VALUES (?1, ?2, 1, ?3, ?4)",
            params![id, name, palette_json, now()],
        )?;
        Ok(())
    }

    pub fn count_themes(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM themes", [], |r| r.get(0))?)
    }
}

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

/// Turn a free-text query into a safe FTS5 prefix query.
fn fts_query(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned
        .split_whitespace()
        .map(|t| format!("{t}*"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Storage {
        Storage::open_in_memory().unwrap()
    }

    #[test]
    fn migrations_apply() {
        let s = store();
        assert!(s.schema_version().unwrap() >= 1);
    }

    #[test]
    fn history_record_and_search() {
        let s = store();
        s.upsert_session("sess-1", None, 80, 24).unwrap();
        for cmd in [
            "cargo build",
            "cargo test",
            "git status",
            "git commit -m wip",
        ] {
            s.record_command(
                "sess-1",
                &HistoryEntry {
                    command: cmd.into(),
                    cwd: None,
                    exit_code: Some(0),
                    duration_ms: Some(12),
                },
            )
            .unwrap();
        }
        let git = s.search_history("git", 10).unwrap();
        assert_eq!(git.len(), 2);
        let all = s.search_history("", 10).unwrap();
        assert_eq!(all.len(), 4);
        // most recent first
        assert_eq!(all[0], "git commit -m wip");
    }

    #[test]
    fn workspace_roundtrip() {
        let s = store();
        s.save_workspace("ws1", "Web Dev", r#"{"split":"v"}"#)
            .unwrap();
        let (name, layout) = s.load_workspace("ws1").unwrap().unwrap();
        assert_eq!(name, "Web Dev");
        assert_eq!(layout, r#"{"split":"v"}"#);
        // update path
        s.save_workspace("ws1", "Web Dev 2", "{}").unwrap();
        assert_eq!(s.load_workspace("ws1").unwrap().unwrap().0, "Web Dev 2");
        assert_eq!(s.list_workspaces().unwrap().len(), 1);
    }

    #[test]
    fn kv_roundtrip() {
        let s = store();
        assert_eq!(s.kv_get("channel").unwrap(), None);
        s.kv_set("channel", "stable").unwrap();
        assert_eq!(s.kv_get("channel").unwrap(), Some("stable".into()));
        s.kv_set("channel", "beta").unwrap();
        assert_eq!(s.kv_get("channel").unwrap(), Some("beta".into()));
    }

    #[test]
    fn theme_seeding_idempotent() {
        let s = store();
        s.seed_theme("nord", "Nord", "{}").unwrap();
        s.seed_theme("nord", "Nord", "{}").unwrap();
        assert_eq!(s.count_themes().unwrap(), 1);
    }
}
