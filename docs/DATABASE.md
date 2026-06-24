# NovaTerm — Database Schema

SQLite (WAL mode, `synchronous=NORMAL`, `foreign_keys=ON`). One DB file at
`%APPDATA%/NovaTerm/novaterm.db`. Migrations are versioned and applied at
startup (see `nova-storage::migrations`).

## ER overview

```
profiles ──< sessions >── workspaces ──< workspace_panes
   │             │
   │             └──< command_history
   │             └──< session_snapshots ──< snapshot_frames   (terminal replay)
themes
bookmarks
analytics_events
kv (settings sync cursor, misc)
```

## Tables

```sql
-- Shell profiles (PowerShell, CMD, WSL, ...)
CREATE TABLE profiles (
    id          TEXT PRIMARY KEY,             -- uuid
    name        TEXT NOT NULL,
    shell       TEXT NOT NULL,                -- executable
    args        TEXT NOT NULL DEFAULT '[]',   -- json array
    cwd         TEXT,
    env         TEXT NOT NULL DEFAULT '{}',   -- json object
    icon        TEXT,
    color       TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- Workspaces: a saved arrangement of tabs/panes
CREATE TABLE workspaces (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    layout      TEXT NOT NULL DEFAULT '{}',   -- json: split tree
    startup_cmds TEXT NOT NULL DEFAULT '[]',  -- json array of strings
    env_preset  TEXT NOT NULL DEFAULT '{}',   -- json object
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- A concrete terminal session (may be live or hibernated)
CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    profile_id   TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    title        TEXT,
    cwd          TEXT,
    cols         INTEGER NOT NULL DEFAULT 80,
    rows         INTEGER NOT NULL DEFAULT 24,
    state        TEXT NOT NULL DEFAULT 'active', -- active|hibernated|exited
    created_at   INTEGER NOT NULL,
    last_active  INTEGER NOT NULL
);

CREATE INDEX idx_sessions_workspace ON sessions(workspace_id);

-- Panes inside a workspace layout (denormalized for fast restore)
CREATE TABLE workspace_panes (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    profile_id   TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    parent_id    TEXT,                          -- null = root
    split_dir    TEXT,                          -- horizontal|vertical|null(leaf)
    ratio        REAL NOT NULL DEFAULT 0.5,
    order_idx    INTEGER NOT NULL DEFAULT 0
);

-- Persistent command history (across sessions, searchable)
CREATE TABLE command_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    profile_id  TEXT,
    command     TEXT NOT NULL,
    cwd         TEXT,
    exit_code   INTEGER,
    duration_ms INTEGER,
    ts          INTEGER NOT NULL
);

CREATE INDEX idx_history_ts ON command_history(ts DESC);
CREATE VIRTUAL TABLE command_history_fts USING fts5(
    command, content='command_history', content_rowid='id'
);

-- Bookmarks: pinned commands / scroll positions
CREATE TABLE bookmarks (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    command     TEXT,
    session_id  TEXT,
    line        INTEGER,
    created_at  INTEGER NOT NULL
);

-- Session snapshots + replay (terminal replay / session recording)
CREATE TABLE session_snapshots (
    id          TEXT PRIMARY KEY,
    session_id  TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    label       TEXT,
    cols        INTEGER NOT NULL,
    rows        INTEGER NOT NULL,
    grid_blob   BLOB,                          -- compressed grid snapshot
    created_at  INTEGER NOT NULL
);

-- Frame deltas for replay/timeline (one row per recorded tick)
CREATE TABLE snapshot_frames (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id TEXT NOT NULL REFERENCES session_snapshots(id) ON DELETE CASCADE,
    offset_ms   INTEGER NOT NULL,              -- ms since recording start
    diff_blob   BLOB NOT NULL                  -- compressed FrameDiff (cbor/zstd)
);

CREATE INDEX idx_frames_snapshot ON snapshot_frames(snapshot_id, offset_ms);

-- Themes (built-in seeded + user)
CREATE TABLE themes (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    builtin     INTEGER NOT NULL DEFAULT 0,
    palette     TEXT NOT NULL,                 -- json: ansi + ui colors
    updated_at  INTEGER NOT NULL
);

-- Analytics (local-only by default; opt-in sync)
CREATE TABLE analytics_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT NOT NULL,                 -- startup|command|render_stats|...
    payload     TEXT NOT NULL DEFAULT '{}',    -- json
    ts          INTEGER NOT NULL
);
CREATE INDEX idx_analytics_kind_ts ON analytics_events(kind, ts DESC);

-- Generic key/value (settings-sync cursor, feature flags, update channel)
CREATE TABLE kv (
    k           TEXT PRIMARY KEY,
    v           TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- Schema version
CREATE TABLE schema_version (version INTEGER NOT NULL);
```

## Notes

- **Replay/timeline** is reconstructed by loading the base `grid_blob` then
  applying `snapshot_frames` in `offset_ms` order — the same `FrameDiff` apply
  path the live renderer uses, so recording and playback share one code path.
- **History FTS5** powers instant fuzzy command search in the palette.
- **Settings sync** stores a Lamport-ish cursor in `kv`; conflict resolution is
  last-writer-wins per key with a per-device id (see SECURITY.md §sync).
- Compression: `diff_blob`/`grid_blob` are CBOR then zstd. Spilled scrollback
  (millions of lines) lands in a separate append-only `scrollback_spill` file,
  not this DB, to keep the main DB small.
