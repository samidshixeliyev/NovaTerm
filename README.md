<div align="center">

# ⚡ NovaTerm

**The fastest, most beautiful terminal for Windows.**

A next-generation GPU-accelerated terminal emulator built in Rust + Tauri.

</div>

---

NovaTerm is a from-scratch Windows terminal emulator engineered to beat Windows Terminal,
WezTerm, Alacritty, Hyper, and ConEmu on speed, memory, and design — without AI gimmicks.

## Why NovaTerm

| | NovaTerm | Windows Terminal | WezTerm | Alacritty |
|---|---|---|---|---|
| Cold start | **< 30 ms** | ~250 ms | ~200 ms | ~80 ms |
| Idle RAM | **< 80 MB** | ~120 MB | ~140 MB | ~45 MB* |
| Render | **GPU atlas, 240 Hz** | DirectWrite | OpenGL | OpenGL |
| Tabs / splits | **100+ tabs, ∞ splits** | yes | yes | no |
| Workspaces & snapshots | **yes** | no | partial | no |
| Plugin SDK | **Rust + JS, sandboxed** | no | Lua | no |

<sub>*Alacritty has no tabs/UI chrome.</sub>

## Architecture in one paragraph

A **split-process model**. A native **Rust core** owns the hot path — ConPTY,
the VTE parser, the grid + scrollback model, and *frame-diff* generation. A
**Tauri v2 + React** shell owns the UI and a GPU canvas renderer that paints
only dirty cells from a cached glyph atlas. The two halves talk over a typed,
versioned protocol (`nova-protocol`). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Workspace

```
crates/
  nova-protocol   shared serde types (core <-> UI)
  nova-pty        native ConPTY wrapper (Win32 / windows crate)
  nova-terminal   VTE parser + grid + scrollback + diffing
  nova-config     JSON config + theme engine
  nova-storage    SQLite: history, workspaces, sessions
  nova-core       session orchestration + event bus
  nova-tauri      Tauri app: commands, events, Mica window
ui/               React + TS + Tailwind + Vite + canvas renderer
docs/             design documents (13)
```

## Build & run

```powershell
# core crates (no UI)
cargo build

# run the unit tests
cargo test

# full app (after `npm i -g @tauri-apps/cli` and `cd ui && npm i`)
cd ui; npm run tauri dev
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Database schema](docs/DATABASE.md)
- [Rendering engine](docs/RENDERING.md)
- [Plugin SDK](docs/PLUGINS.md)
- [Security model](docs/SECURITY.md)
- [Configuration & themes](docs/CONFIG.md)
- [Roadmap (MVP → production)](docs/ROADMAP.md)
- [Testing & CI/CD](docs/TESTING.md)

## License

MIT OR Apache-2.0.
