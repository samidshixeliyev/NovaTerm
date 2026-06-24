# NovaTerm — Roadmap

## State management (UI)

- **Core is the source of truth** for terminal state; the UI mirrors it.
- React app state via **Zustand** stores, sliced by concern:
  `sessionStore` (tabs/panes/focus), `configStore`, `themeStore`,
  `paletteStore`, `workspaceStore`. Renderer cell buffers live *outside* React
  (plain typed arrays owned by the canvas controller) — React only holds
  metadata (titles, ids, layout), never per-cell data, to avoid re-render storms.
- Cross-cutting effects (frame events, config reload) are wired in a thin
  `bridge` layer subscribing to Tauri events and dispatching to stores.

## MVP roadmap (target: a daily-drivable terminal)

**M0 — Foundation** ✅ (this milestone)
- Workspace, protocol types, native ConPTY, VTE grid model, session manager.

**M1 — Vertical slice**
- Tauri commands/events; React shell; canvas renderer with glyph atlas; type in
  PowerShell and see output; resize; basic theme.

**M2 — Core UX**
- Tabs (new/close/switch), split panes, copy/paste (bracketed), scrollback +
  scroll, search (plain), config + theme loading, profiles (pwsh/cmd/wsl/gitbash).

**M3 — Power features**
- Command palette, command history (SQLite + FTS), workspaces + layout
  save/restore, settings UI, keybinding engine.

## Production roadmap

**P1 — Performance & polish**
- WebGPU backend, dirty-region tuning, tab hibernation, scrollback disk spill,
  Mica/Acrylic, animations, snap layouts, 240 Hz validation, startup < 30 ms.

**P2 — Differentiators**
- Workspace snapshots, terminal replay + timeline view, session recording,
  broadcast input, pane sync, regex search, bookmarks, hyperlinks, image support.

**P3 — Ecosystem**
- Plugin host + SDK (Rust/WASM + JS), permission UI, hot reload, marketplace,
  bundled plugins (Docker/K8s/AWS/Azure/PostgreSQL/MySQL/Redis/GitHub).

**P4 — Pro tooling & sync**
- Built-in dev tools (git status, file explorer, process/network/resource
  monitors, SSH manager), cross-device settings sync, performance profiler,
  terminal analytics, built-in updater.

## Differentiating features → where they live

| Feature | Crate(s) |
|---|---|
| Workspace snapshots | nova-storage + nova-core |
| Tab hibernation | nova-core (drops parser/GPU state, keeps compact scrollback) |
| Terminal replay / timeline | nova-storage (snapshot_frames) + renderer apply path |
| Session recording | nova-core + nova-storage |
| Settings sync | nova-storage + nova-config (E2E encrypted) |
| Performance profiler | nova-core diagnostics + UI overlay |
| Terminal analytics | nova-storage (analytics_events) |
| Built-in updater | nova-tauri (signed manifest) |

## Definition of done per milestone

Every milestone ships with: unit tests for new core logic, a manual smoke
checklist, no clippy warnings, and updated docs.
