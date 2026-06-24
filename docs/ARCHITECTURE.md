# NovaTerm — System Architecture

## 1. Design principles

1. **Hot path in native Rust.** Bytes from the shell must reach pixels with the
   fewest copies and zero JS on the critical path. Parsing, grid mutation and
   diffing happen in Rust on dedicated threads.
2. **The UI never blocks on the shell, and the shell never blocks on the UI.**
   They are decoupled by bounded lock-free channels; backpressure is explicit.
3. **Send diffs, not frames.** The core computes the minimal set of changed
   cells per tick and ships only those to the renderer.
4. **Everything is a session.** A tab, a split pane, a hibernated buffer, and a
   recorded replay are all views over the same `Session` abstraction.
5. **Native Windows, modern UI.** ConPTY + Win32 for the system layer; Fluent /
   Mica for the chrome.

## 2. Process & thread model

```
┌──────────────────────────────────────────────────────────────────────┐
│  NovaTerm process (Tauri)                                              │
│                                                                        │
│  ┌────────────────────────┐         ┌──────────────────────────────┐  │
│  │  WebView (UI process)  │  IPC    │  Rust core (native)          │  │
│  │  React + TS + Tailwind │◀──────▶ │                              │  │
│  │  Canvas/WebGL renderer │ events  │  ┌────────────────────────┐  │  │
│  └────────────────────────┘ + cmds │  │ SessionManager         │  │  │
│                                     │  └───────────┬────────────┘  │  │
│                                     │   per session│               │  │
│                                     │   ┌──────────▼───────────┐   │  │
│                                     │   │ PTY read thread      │   │  │
│                                     │   │  ConPTY ──bytes──▶   │   │  │
│                                     │   └──────────┬───────────┘   │  │
│                                     │   ┌──────────▼───────────┐   │  │
│                                     │   │ Parser/grid thread   │   │  │
│                                     │   │  vte → Grid → Diff   │   │  │
│                                     │   └──────────┬───────────┘   │  │
│                                     │   coalesce @ frame cadence   │  │
│                                     └──────────────┼───────────────┘  │
│                                          FrameDiff │ (Tauri event)     │
│                                  ◀─────────────────┘                   │
└──────────────────────────────────────────────────────────────────────┘
        ▲ user input (keys/mouse/resize) ── Tauri command ──┘
```

**Threads per session**
- *PTY reader*: blocking `ReadFile` on the ConPTY output pipe → pushes raw byte
  chunks into a `crossbeam` channel. One OS thread (cheap; blocked on I/O).
- *Parser/grid*: drains byte chunks, feeds `vte::Parser`, mutates the `Grid`,
  marks dirty rows. Coalesces work and, on a frame tick (default 8 ms / 120 Hz,
  configurable to 240 Hz), produces a `FrameDiff`.
- *Writer*: input from UI is written to the ConPTY input pipe (async via Tokio).

Global threads: a Tokio runtime for storage/IO, the Tauri event loop (UI), and a
small scheduler for hibernation/snapshot timers.

## 3. Data flow (keystroke → glyph)

1. User presses a key → React captures it → `write_session` Tauri command.
2. Core writes bytes to ConPTY input pipe.
3. Shell processes, emits output → ConPTY output pipe.
4. PTY reader thread reads chunk → channel.
5. Parser thread feeds `vte`, mutates `Grid`, marks dirty rows.
6. On frame tick, `Grid::take_diff()` returns changed cells + cursor + scroll.
7. Core emits `frame://<session>` Tauri event with the `FrameDiff`.
8. Renderer applies the diff to its cell buffer and repaints dirty regions from
   the glyph atlas.

Round-trip budget target: **< 6 ms** at 120 Hz on commodity hardware.

## 4. Event system

A typed event bus (`nova-core::events`) backed by `tokio::sync::broadcast` for
fan-out and `crossbeam-channel` for the per-session hot path. Event categories:

- **Session**: `Spawned`, `Exited`, `TitleChanged`, `Bell`, `OscHyperlink`.
- **Render**: `FrameDiff`, `FullRepaintRequested`.
- **Lifecycle**: `Hibernated`, `Restored`, `SnapshotSaved`.
- **App**: `ConfigReloaded`, `ThemeChanged`, `PluginEvent`.

UI subscribes via Tauri events; plugins subscribe via the plugin host with a
permission-filtered view.

## 5. PTY architecture

`nova-pty` wraps the Win32 pseudoconsole API directly (no shelling out):
`CreatePseudoConsole`, `ResizePseudoConsole`, `ClosePseudoConsole`, plus
`CreateProcess` with `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`. We own two anonymous
pipes (in/out). This gives us:

- Exact control over resize semantics (no flicker on column changes).
- Direct handle lifetime management (clean child teardown, no zombies).
- A path to future conhost-less optimizations.

See [`crates/nova-pty`](../crates/nova-pty) and §5 of [RENDERING.md](RENDERING.md).

## 6. Storage system

`nova-storage` uses SQLite (WAL mode) for durable state: command history,
workspaces, saved layouts, session snapshots, and analytics. JSON files hold
user-editable config and themes (`nova-config`). Rationale: SQLite for
high-write/query data (history, analytics, replay frames), JSON for
human-edited, diffable config. Schema: [DATABASE.md](DATABASE.md).

## 7. Crate dependency graph

```
nova-protocol ──┬─────────────┬──────────────┬────────────┐
                ▼             ▼              ▼            ▼
            nova-pty   nova-terminal   nova-config   nova-storage
                └────────┬─────┴──────────┬───────────┘
                         ▼                 ▼
                      nova-core ───────────┘
                         ▼
                      nova-tauri  ──IPC──▶  ui/ (React)
```

`nova-protocol` is the only crate every layer depends on, keeping the wire
contract central and versioned.

## 8. Performance strategy (mapped to targets)

| Target | Mechanism |
|---|---|
| < 30 ms startup | Lazy SQLite open, no synchronous plugin load, pre-warmed atlas, defer non-visible tabs |
| < 80 MB idle | Scrollback as packed ring buffer (8 bytes/cell), tab hibernation, single shared atlas |
| 100+ tabs | Hibernate inactive tabs (drop GPU + parser state, keep compact scrollback) |
| 240 Hz | Frame-tick coalescing decoupled from PTY rate; dirty-region repaint |
| Millions of lines | Virtualized scrollback ring + on-disk overflow spill via storage |
| Zero-lag scroll | Renderer owns its cell buffer; scroll is a viewport offset, no IPC |

## 9. Failure & resilience

- PTY death → `Exited` event, buffer frozen, tab marked, optional auto-restart.
- Parser panic isolation: parser thread is restartable; a poisoned session is
  quarantined, not the whole app.
- Renderer falls back from WebGPU → WebGL → 2D canvas automatically.
- Config parse error → keep last-good config, surface a non-blocking toast.
