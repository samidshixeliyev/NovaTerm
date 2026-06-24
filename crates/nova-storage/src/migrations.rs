//! Versioned schema migrations, applied in order at startup.

use rusqlite::Connection;

/// Each migration is `(version, sql)`. Add new migrations by appending; never
/// edit a shipped one.
const MIGRATIONS: &[(i64, &str)] = &[(1, V1)];

pub fn run(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")?;
    let current: i64 = conn
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .unwrap_or(0);

    let tx = conn.transaction()?;
    for &(version, sql) in MIGRATIONS {
        if version > current {
            tx.execute_batch(sql)?;
        }
    }
    let latest = MIGRATIONS.last().map(|m| m.0).unwrap_or(0);
    if latest > current {
        tx.execute("DELETE FROM schema_version", [])?;
        tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [latest])?;
    }
    tx.commit()?;
    Ok(())
}

const V1: &str = r#"
CREATE TABLE profiles (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, shell TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]', cwd TEXT, env TEXT NOT NULL DEFAULT '{}',
    icon TEXT, color TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);

CREATE TABLE workspaces (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, layout TEXT NOT NULL DEFAULT '{}',
    startup_cmds TEXT NOT NULL DEFAULT '[]', env_preset TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    profile_id TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT, cwd TEXT, cols INTEGER NOT NULL DEFAULT 80, rows INTEGER NOT NULL DEFAULT 24,
    state TEXT NOT NULL DEFAULT 'active', created_at INTEGER NOT NULL, last_active INTEGER NOT NULL
);
CREATE INDEX idx_sessions_workspace ON sessions(workspace_id);

CREATE TABLE workspace_panes (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    profile_id TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    parent_id TEXT, split_dir TEXT, ratio REAL NOT NULL DEFAULT 0.5,
    order_idx INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE command_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    profile_id TEXT, command TEXT NOT NULL, cwd TEXT,
    exit_code INTEGER, duration_ms INTEGER, ts INTEGER NOT NULL
);
CREATE INDEX idx_history_ts ON command_history(ts DESC);
CREATE VIRTUAL TABLE command_history_fts USING fts5(
    command, content='command_history', content_rowid='id'
);

CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY, label TEXT NOT NULL, command TEXT,
    session_id TEXT, line INTEGER, created_at INTEGER NOT NULL
);

CREATE TABLE session_snapshots (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    label TEXT, cols INTEGER NOT NULL, rows INTEGER NOT NULL,
    grid_blob BLOB, created_at INTEGER NOT NULL
);

CREATE TABLE snapshot_frames (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id TEXT NOT NULL REFERENCES session_snapshots(id) ON DELETE CASCADE,
    offset_ms INTEGER NOT NULL, diff_blob BLOB NOT NULL
);
CREATE INDEX idx_frames_snapshot ON snapshot_frames(snapshot_id, offset_ms);

CREATE TABLE themes (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, builtin INTEGER NOT NULL DEFAULT 0,
    palette TEXT NOT NULL, updated_at INTEGER NOT NULL
);

CREATE TABLE analytics_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}', ts INTEGER NOT NULL
);
CREATE INDEX idx_analytics_kind_ts ON analytics_events(kind, ts DESC);

CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL, updated_at INTEGER NOT NULL);
"#;
