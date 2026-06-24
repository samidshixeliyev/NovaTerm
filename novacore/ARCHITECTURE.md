# NovaCore — Architecture

NovaCore is **not a terminal emulator and not a shell frontend**. It is a new
command platform: a native runtime that combines a *shell*, a *terminal*, a
*file manager*, a *process manager*, and a *workspace manager* into one
application. Traditional shells (PowerShell, CMD, Bash, Zsh, Nushell, WSL) are
**optional adapter plugins**, never the architecture.

The organizing idea: **everything is a `Value`.** Commands consume and produce
structured values (records, tables, typed objects like `FileEntry`), not text.
Text is merely one *view* of a value. This is what lets `ls` return
`Files[] / Directories[] / Metadata[]` that the UI can render as a table, tree,
grid, cards, or timeline — and lets `ls | where size > 1mb | sort-by modified`
operate on real fields instead of parsing strings.

```
                         ┌──────────────────────────────────────────┐
                         │                NovaCore                   │
                         │            (nova-engine)                  │
  NovaLang source ─────▶ │  Parser → AST → Evaluator                 │
                         │     │                                     │
                         │     ▼                                     │
                         │  Command Engine ◀── Registry (builtins +  │
                         │     │                 plugin commands)    │
                         │     │  Value in ──▶ Value out (pipeline)   │
                         │     ▼                                     │
   Session Manager ──────┼──▶ Sessions  Workspaces  History  EventBus│
   Process Manager ──────┤    Process table   VFS providers          │
   Plugin Runtime  ──────┘                                           │
                         └───────────────┬──────────────────────────┘
                                         │ Values + view hints
                                         ▼
                         ┌──────────────────────────────────────────┐
                         │       Native GUI (nova-shell)             │
                         │  Layout engine → Widget tree → nova-ui    │
                         │  Glyph atlas + instanced quads → nova-gpu │
                         │  winit window  ·  wgpu device             │
                         └──────────────────────────────────────────┘
```

No Electron, Tauri, WebView, HTML, CSS, or JS anywhere. The UI is Rust + wgpu.

---

## 1. Crate map (process model is single-process, multi-threaded)

```
novacore/
├─ crates/
│  ├─ nova-value     # the structured data model (Value, Type, views) — the spine
│  ├─ nova-lang      # NovaLang: lexer + parser → AST
│  ├─ nova-cmd       # Command trait, registry, invocation, pipeline plumbing, builtins
│  ├─ nova-vfs       # virtual filesystem layer (provider trait + local/git/ssh/mem)
│  ├─ nova-proc      # process manager (spawn native exes, track, reap, structured I/O)
│  ├─ nova-history   # command history engine (structured, searchable, persisted)
│  ├─ nova-bus       # event bus (typed pub/sub, lock-free hot path)
│  ├─ nova-engine    # NovaCore: sessions, workspaces, evaluator, plugin runtime
│  ├─ nova-gpu       # wgpu device, glyph atlas, instanced quad/text pipeline
│  ├─ nova-ui        # retained widget tree + flexbox-style layout engine + value views
│  └─ nova-shell     # the application: winit window, input, wiring engine ↔ UI
└─ plugins/          # out-of-process / dynamic plugins (shells, git, ssh, pkg, cloud)
```

Dependency direction (acyclic):

```
nova-value ◀─ everything
nova-lang  ◀─ nova-engine
nova-cmd   ◀─ nova-engine          nova-vfs ◀─ nova-cmd, nova-engine
nova-proc  ◀─ nova-cmd, nova-engine
nova-history,nova-bus ◀─ nova-engine
nova-engine ◀─ nova-shell
nova-gpu ◀─ nova-ui ◀─ nova-shell
```

---

## 2. The Value model (`nova-value`) — the spine

```rust
enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Duration(i64),          // nanoseconds
    Filesize(u64),          // bytes, rendered 1.2 MB
    Date(i64),              // unix nanos
    List(Vec<Value>),
    Record(Record),         // ordered key→Value
    Table(Table),           // columns + rows of Records (homogeneous-ish)
    Stream(StreamId),       // lazy/external (e.g. a shell adapter's PTY output)
    Error(ValueError),
    Custom(Arc<dyn CustomValue>), // typed objects: FileEntry, Process, GitStatus…
}
```

* **Structured by default.** `ls` yields a `Table` of `FileEntry` customs, each
  with `name, path, kind, size: Filesize, modified: Date, mode, …`.
* **Views are hints, not data.** A `Value` carries an optional `View`
  (`Auto | Table | Tree | Grid | Cards | Timeline | Raw`). The UI picks a
  renderer from the value's shape + hint; the same data renders many ways.
* **Spans** thread back to NovaLang source for precise diagnostics.
* `CustomValue` lets plugins add first-class typed values that still flow through
  `get`, `where`, `sort-by`, and the renderers via a small reflection trait
  (`fields()`, `get_field()`, `type_name()`).

This is the difference from every text shell: pipelines move *typed data*.

---

## 3. Runtime / command system (`nova-lang`, `nova-cmd`, `nova-engine`)

### NovaLang (the custom command language)
A small, predictable language designed for structured data:

```
ls ~/src
ls | where { $it.size > 1mb } | sort-by modified | first 10
def deploy [env] { build $env | upload --to $env }
let big = (find . -name "*.log" | where size > 10mb)
git status | where staged == false
```

Pipeline grammar (recursive-descent + Pratt for expressions):

```
pipeline   := expr ('|' expr)*
expr       := command | literal | '(' pipeline ')' | binary | block
command    := bareword (arg)*
arg        := flag | literal | '{' pipeline '}' | '$' ident
literal    := int | float | string | list | record | bool | null | path
```

The AST is evaluated by **nova-engine**: each pipeline stage is a `Command`
invocation. Stage *N*'s output `Value` becomes stage *N+1*'s input (`$in`).

### Command engine
```rust
trait Command: Send + Sync {
    fn signature(&self) -> Signature;     // name, args, flags, in/out types
    fn run(&self, ctx: &mut EvalCtx, input: Value, call: Call) -> Result<Value>;
}
```
* **Registry** maps names → `Arc<dyn Command>`; built-ins and plugin commands are
  registered identically. Resolution order: builtin → plugin → external exe
  (via the process manager) → shell adapter fallback.
* **Structured args:** `Signature` declares positional args, typed flags, and
  rest args; the engine binds `Call` from the AST and type-checks before `run`.
* **Streaming:** `Value::Stream` lets large/external output flow lazily so a
  `cat huge.log | where ...` never materializes the whole file.

### Running native executables directly
If a bareword isn't a registered command, the engine resolves it on `PATH` and
runs it through **nova-proc** — no shell required. stdout is exposed as a
`Stream`; if `--structured` (or a known parser) applies, it's lifted into a
`Table`. This is how NovaCore "runs anything" without being a shell frontend.

### Shells as adapters (plugins, not the core)
A `ShellAdapter` is a plugin that owns an external interpreter over a PTY:
```rust
trait ShellAdapter { fn launch(&self, ...) -> ShellSession; }  // pwsh/bash/zsh/wsl
```
Its output is a `Stream` value; it integrates with history/sessions like any
command but is entirely optional and sandboxed by the plugin runtime.

---

## 4. Plugin system (`nova-engine::plugin`)

Two tiers, capability-gated and hot-reloadable:

| Tier | Runtime | Use |
|---|---|---|
| **Native dynamic** | `cdylib` loaded via stable C ABI (`PluginVTable`) | high-perf commands, custom values, views |
| **Sandboxed** | WASM (Wasmtime, WASI subset) | untrusted/marketplace plugins, fuel-metered |

A plugin registers: commands, custom value types, views, VFS providers, and
shell adapters. Capabilities (`fs.read`, `net`, `proc.spawn`, `ui.view`) are
declared in a manifest and enforced at the engine boundary. Hot reload drains
in-flight calls, swaps the module, and re-registers. (Design mirrors and
supersedes the earlier prototype's `docs/PLUGINS.md`.)

---

## 5. Virtual filesystem layer (`nova-vfs`)

```rust
trait VfsProvider: Send + Sync {
    fn scheme(&self) -> &str;                      // "file", "git", "ssh", "mem", "zip"
    fn read_dir(&self, path: &VfsPath) -> Result<Vec<Entry>>;
    fn open(&self, path: &VfsPath, mode: OpenMode) -> Result<Box<dyn VfsFile>>;
    fn metadata(&self, path: &VfsPath) -> Result<Metadata>;
}
```
Paths are URIs (`file:///…`, `ssh://host/…`, `git://repo@rev/…`). Built-in file
operations (`ls`, `cp`, `mv`, `rm`, `find`, `cat`, `stat`) and **built-in
search** are written against the VFS, so they work uniformly over local disk,
remote SSH, archives, and in-memory overlays. Git/SSH integrations expose
providers *and* commands.

---

## 6. Process model (`nova-proc`)

* Single OS process for NovaCore; native commands run on the thread pool.
* External executables are children tracked in a **process table** (pid, argv,
  cwd, env, status, rusage). NovaCore reaps them, captures stdout/stderr as
  `Stream`s, and surfaces a `Process` custom value to `ps`, `kill`, `wait`,
  `jobs`.
* No PowerShell anywhere in the spawn path — `nova-proc` calls the OS directly
  (`CreateProcessW` / `posix_spawn`) and owns the pipes.
* Job control: background (`&`), foreground, signals, and per-job event streams
  on the bus.

---

## 7. Rendering engine (`nova-gpu`)

A from-scratch GPU renderer (no text/UI framework dependency for drawing):

```
Frame:  collect draw list (rects, glyph quads, images)
        → upload one instance buffer (SoA)
        → 1–3 instanced draw calls (rects, text, images)
Text:   font rasterized (ab_glyph/fontdue) → coverage atlas (R8) on a wgpu texture
        glyph = textured quad with fg/bg/flags; LRU eviction; subpixel bins
Damage: only changed regions re-recorded; idle frames skipped (0% GPU at rest)
Backend: wgpu (Vulkan/DX12/Metal); vsync via surface; HiDPI aware
```

The renderer knows nothing about terminals or widgets — it draws rects, glyphs,
and images from a flat instance list that `nova-ui` produces.

---

## 8. UI framework + layout engine (`nova-ui`)

* **Retained widget tree.** Widgets (`Panel`, `Text`, `TableView`, `TreeView`,
  `GridView`, `CardsView`, `TimelineView`, `Input`, `Tabs`, `Splitter`) hold
  state across frames; updates mark dirty subtrees.
* **Layout engine.** A flexbox-style solver (direction, grow/shrink, basis,
  gap, align) computes rects in one top-down pass; integer-snapped for crisp
  text. Splitters and docks build pane layouts.
* **Value views.** Each `View` hint maps to a widget that knows how to render a
  `Value`: `Table→TableView`, `Record→Cards`, hierarchical `Custom→TreeView`,
  time-keyed rows→`TimelineView`. The same `ls` result switches view live.
* Immediate-mode-style event handling on top of the retained tree (hit-testing
  from the layout rects), so it stays simple without a vDOM.

---

## 9. Window & workspace management (`nova-engine`, `nova-shell`)

* **Window**: one winit window hosts the whole app; native decorations optional;
  panes are internal (the layout engine owns splits, not the OS).
* **Session**: one running context = cwd, env, history cursor, scrollback of
  `Value`s, and an optional shell adapter. Sessions are cheap and serializable.
* **Workspace**: a saved tree of panes → sessions, plus startup pipelines and an
  env preset. Workspaces snapshot/restore (including the structured scrollback),
  enabling "reopen exactly where I was" and tab hibernation (drop GPU + live
  process state, keep the Value scrollback compactly).

---

## 10. Event bus (`nova-bus`)

Typed pub/sub. Hot path (frame/process output) uses `crossbeam` channels;
fan-out (config/theme/plugin) uses a broadcast. Event kinds: `SessionSpawned`,
`ProcessExited`, `ValueProduced`, `HistoryAppended`, `WorkspaceChanged`,
`PluginEvent`, `Redraw`. The UI subscribes to `Redraw`/`ValueProduced`; plugins
get a capability-filtered view.

---

## 11. Memory model

* `Value` is cheap to move; large/shared payloads (`Custom`, big `Bytes`,
  `Table` columns) sit behind `Arc` for O(1) clone in pipelines.
* **Streaming over materializing**: `Stream` values pull in chunks; bounded
  channels apply backpressure so a fast producer can't OOM the UI.
* **Scrollback** stores Values, not pixels; old entries compact to a summary
  (`Table` → row count + schema) and can spill to disk via a history store.
* Glyph atlas and instance buffers are pooled and reused; no per-frame alloc on
  the steady-state path.

---

## 12. Threading model

```
main thread        winit event loop + wgpu submit (UI only; never blocks on I/O)
engine thread      evaluates pipelines; owns Registry, Sessions, Workspaces
worker pool        command execution, VFS I/O, search, git (rayon-style)
proc reaper        waits on children, pumps their pipes into Streams
plugin host        WASM/native plugin calls, isolated; one queue per plugin
```
Threads communicate over `nova-bus` channels and `Arc`-shared registries
(`parking_lot` locks for the few shared maps). The UI thread only ever receives
finished `Value`s / draw lists — input → command is async, so the window stays
at framerate under heavy command load.

---

## 13. Production-grade implementation plan

**Phase 0 — Engine core (headless, fully testable)** ← *this milestone*
1. `nova-value` — Value/Type/View, CustomValue, FileEntry, to-text + table view.
2. `nova-lang` — lexer + parser → AST (pipelines, args, flags, literals, blocks).
3. `nova-cmd` — Command trait, Signature, registry, Call binding; builtins
   (`ls, pwd, cd, echo, where, get, sort-by, first, lines`).
4. `nova-vfs` / `nova-proc` / `nova-history` / `nova-bus` — providers, process
   table, structured history, typed bus.
5. `nova-engine` — evaluator tying it together; `parse → eval → Value`.
   *Gate:* `ls | where size > 0 | sort-by name | first 3` returns a correct
   `Table` in a unit test. (Done in this milestone.)

**Phase 1 — Native GUI**
6. `nova-gpu` — wgpu device, glyph atlas, instanced text/rect pipeline.
7. `nova-ui` — layout engine + widget tree + TableView/TreeView/Cards/Timeline.
8. `nova-shell` — winit window, input → engine, Value scrollback rendering,
   command input box, view switcher.

**Phase 2 — Platform features**
9. Process/job control UI, file-manager pane (TreeView over VFS), built-in
   search, git status/diff views, SSH provider + client, package-manager
   adapters, task automation (`def`/scripts/watchers).

**Phase 3 — Plugins & polish**
10. Native + WASM plugin runtime, capability manifests, hot reload, marketplace;
    shell adapters (pwsh/bash/zsh/nu/wsl) as the first plugins; workspace
    snapshots, themes, settings, updater.

**Quality bar:** every engine crate ships with unit + property tests, `clippy
-D warnings`, `fmt`, and benches for the pipeline hot path. The GUI crates carry
golden-layout tests; the renderer has a headless wgpu smoke test in CI.
```
```
