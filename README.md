<div align="center">

# ⚡ NovaTerm — A Fast, Beautiful Terminal for Windows

**The GPU-accelerated, Rust-powered terminal emulator for Windows.**
Auto-detects every shell on your PC — PowerShell, CMD, WSL, Git Bash, Nushell — with a gorgeous, modern UI.

[![Release](https://img.shields.io/github/v/release/samidshixeliyev/NovaTerm?style=flat-square&color=7aa2f7)](https://github.com/samidshixeliyev/NovaTerm/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/samidshixeliyev/NovaTerm/total?style=flat-square&color=9ece6a)](https://github.com/samidshixeliyev/NovaTerm/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078d6?style=flat-square)](https://github.com/samidshixeliyev/NovaTerm/releases)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat-square)](#license)

[**⬇ Download**](https://github.com/samidshixeliyev/NovaTerm/releases/latest) ·
[Features](#-features) ·
[Shells](#-every-shell-auto-detected) ·
[Themes](#-themes) ·
[Build](#-build-from-source)

</div>

---

**NovaTerm** is a from-scratch **Windows terminal emulator** built in **Rust + Tauri**, engineered to beat
**Windows Terminal**, **WezTerm**, **Alacritty**, **Hyper**, and **ConEmu** on speed, memory footprint, and
design — with **no AI gimmicks**. Native **ConPTY**, a **GPU (WebGL) renderer**, a bundled Nerd Font, and a
clean, keyboard-first interface.

> Looking for a fast, modern **Windows Terminal alternative**? NovaTerm opens instantly, sips RAM, renders on
> the GPU, and shows you *only the shells you actually have installed.*

## ✨ Features

- 🖥️ **Every shell, auto-detected** — PowerShell 7, Windows PowerShell, Command Prompt, **WSL** (one entry per
  installed distro), **Git Bash**, **Nushell**, MSYS2 and Cygwin. Uninstalled shells never clutter the menu.
- ⚡ **GPU-accelerated rendering** via xterm.js + WebGL with a cached glyph atlas — smooth at high refresh rates.
- 🦀 **Native Rust core** — ConPTY pseudo-console managed by a fast, low-latency byte pump.
- 🔤 **Bundled Nerd Font** (CaskaydiaCove) — powerline glyphs, icons and ligatures render correctly out of the box.
- 🎨 **9 built-in themes** — Tokyo Night, Catppuccin, Dracula, Nord, Gruvbox, One Dark, GitHub Dark, Solarized, Fluent.
- 🗂️ **Tabs** with pinning, live titles, per-shell colors/icons, and a one-click **Restart** when a process exits.
- ⌨️ **Copy / paste** (Ctrl+C / Ctrl+V), a **command palette** (Ctrl+Shift+P), and a polished **Settings** panel.
- 🛡️ **Run as Administrator** — relaunch elevated with a single click.
- 🪟 **Windows 11 Mica** translucency, rounded corners, custom title bar.
- 🚀 **Fast & light** — instant cold start, low idle RAM, no Electron bloat.

## ⬇ Install

1. Download the latest **`NovaTerm_x64-setup.exe`** from the [**Releases**](https://github.com/samidshixeliyev/NovaTerm/releases/latest) page.
2. Run it — it installs to your user profile and creates Start Menu + Desktop shortcuts (no admin required).
3. Prefer no installer? Grab the **portable `.zip`** instead.

> Requires the WebView2 runtime, which is preinstalled on Windows 10/11.

## 🐚 Every shell, auto-detected

NovaTerm probes your machine on startup and lists **only the terminals you actually have** — click `+` for the
default, or the `▾` to pick any detected shell:

| Shell | Detected from |
|---|---|
| **PowerShell 7** (`pwsh`) | `PATH` · `Program Files\PowerShell\7` |
| **Windows PowerShell** | `System32\WindowsPowerShell\v1.0` |
| **Command Prompt** | `System32\cmd.exe` |
| **WSL** | `wsl --list` — **one tab profile per installed distro** |
| **Git Bash** | `Program Files\Git` · `LocalAppData\Programs\Git` |
| **Nushell** (`nu`) | `PATH` · `~/.cargo/bin` |
| **MSYS2 / Cygwin** | `C:\msys64` · `C:\cygwin64` |

No installed shell? It simply doesn't appear — the menu stays clean.

## 🎨 Themes

Tokyo Night · Catppuccin Mocha · Dracula · Nord · Gruvbox Dark · One Dark · GitHub Dark · Solarized Dark · Fluent.
Switch live from **Settings** (Ctrl+,) or the command palette — no restart.

## 🏗️ Architecture

A **split-process model**. A native **Rust core** owns the hot path — ConPTY, the output pump, session
lifecycle, and shell detection. A **Tauri v2 + React** shell owns the UI and a **GPU (WebGL) terminal renderer**
(xterm.js). The two halves talk over a typed, versioned protocol (`nova-protocol`).
See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

```
crates/
  nova-protocol   shared serde types (core <-> UI)
  nova-pty        native ConPTY wrapper (Win32 / windows crate)
  nova-terminal   VTE parser + grid + scrollback
  nova-config     JSON config, theme engine, shell auto-detection
  nova-storage    SQLite: history, workspaces, sessions
  nova-core       session orchestration + PTY pump + event bus
  nova-tauri      Tauri app: commands, events, Mica window
ui/               React + TypeScript + Tailwind + Vite + xterm.js
docs/             design documents
```

## 🔧 Build from source

```powershell
# core crates (no UI) + unit tests
cargo build
cargo test

# install UI deps once
npm --prefix ui install

# dev (hot-reload UI + native core)
cd crates/nova-tauri; npm --prefix ../../ui run tauri dev
```

### Production build (installer + standalone exe)

> ⚠️ Do **not** ship a plain `cargo build` of `nova-tauri`. Without the Tauri CLI it is compiled in *dev* mode
> and tries to load the Vite dev server (`http://localhost:5173`) at runtime — you'll get
> `ERR_CONNECTION_REFUSED`. Always use `tauri build`, which embeds the frontend for a self-contained app.

```powershell
# 1. build the frontend bundle
npm --prefix ui run build

# 2. produce the production binary + NSIS installer
cd crates/nova-tauri
npm --prefix ../../ui run tauri build
```

Outputs:
- `target/release/nova-tauri.exe` — standalone, self-contained (UI embedded; needs WebView2, preinstalled on Win 11).
- `target/release/bundle/nsis/NovaTerm_<ver>_x64-setup.exe` — installer that creates Start Menu + Desktop shortcuts.

## 📚 Documentation

- [Architecture](docs/ARCHITECTURE.md) · [Rendering](docs/RENDERING.md) · [Plugins](docs/PLUGINS.md)
- [Security](docs/SECURITY.md) · [Config & themes](docs/CONFIG.md) · [Database](docs/DATABASE.md)
- [Roadmap](docs/ROADMAP.md) · [Testing & CI/CD](docs/TESTING.md)

## License

MIT OR Apache-2.0.

---

<div align="center">
<sub>

**Keywords:** windows terminal · terminal emulator · windows terminal alternative · conpty · rust terminal ·
gpu terminal · powershell · cmd · wsl · git bash · nushell · xterm.js · tauri · nerd font · console · shell ·
fast terminal · modern terminal · NovaTerm

</sub>
</div>
