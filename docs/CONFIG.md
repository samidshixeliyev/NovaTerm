# NovaTerm — Configuration & Theme Engine

## Files

```
%APPDATA%/NovaTerm/
  config.json        user config (merged over defaults)
  keybindings.json   keybinding overrides
  themes/*.json      user themes
  novaterm.db        state (history/workspaces/snapshots)
```

Config is JSON (diffable, scriptable). It is merged: `defaults → config.json`,
with deep-merge for objects and replace for arrays. Hot-reloaded on file change.

## `config.json` (annotated)

```jsonc
{
  "appearance": {
    "theme": "tokyo-night",
    "font_family": "Cascadia Code",
    "font_size": 13,
    "line_height": 1.2,
    "ligatures": true,
    "cursor": { "style": "bar", "blink": true },
    "window": {
      "material": "mica",        // mica | acrylic | solid
      "rounded": true,
      "padding": [8, 8],
      "opacity": 1.0
    }
  },
  "rendering": {
    "backend": "auto",            // auto | webgpu | webgl | canvas
    "max_fps": 240,
    "frame_tick_ms": 8
  },
  "terminal": {
    "scrollback_lines": 100000,
    "spill_to_disk": true,
    "bell": "visual",
    "copy_on_select": false,
    "word_separators": " \t()[]{}\"'"
  },
  "profiles": {
    "default": "pwsh",
    "list": [
      { "id": "pwsh", "name": "PowerShell", "shell": "pwsh.exe" },
      { "id": "cmd",  "name": "CMD",        "shell": "cmd.exe" },
      { "id": "wsl",  "name": "WSL",        "shell": "wsl.exe" },
      { "id": "gitbash", "name": "Git Bash", "shell": "C:/Program Files/Git/bin/bash.exe" }
    ]
  },
  "behavior": {
    "restore_session": true,
    "hibernate_after_min": 15,
    "confirm_multiline_paste": true
  }
}
```

## Theme schema (`themes/<id>.json`)

```jsonc
{
  "id": "tokyo-night",
  "name": "Tokyo Night",
  "ui": {
    "bg": "#1a1b26", "fg": "#c0caf5",
    "accent": "#7aa2f7", "border": "#2a2e42",
    "tab_active": "#24283b", "tab_inactive": "#16161e"
  },
  "ansi": {
    "black": "#15161e", "red": "#f7768e", "green": "#9ece6a",
    "yellow": "#e0af68", "blue": "#7aa2f7", "magenta": "#bb9af7",
    "cyan": "#7dcfff", "white": "#a9b1d6",
    "bright_black": "#414868", "bright_red": "#f7768e",
    "bright_green": "#9ece6a", "bright_yellow": "#e0af68",
    "bright_blue": "#7aa2f7", "bright_magenta": "#bb9af7",
    "bright_cyan": "#7dcfff", "bright_white": "#c0caf5"
  },
  "cursor": "#c0caf5",
  "selection": "#283457"
}
```

Built-in themes: **Fluent, Nord, Dracula, Catppuccin, Tokyo Night**. Themes are
seeded into the DB on first run and overridable by files in `themes/`.

## Keybindings (`keybindings.json`)

```jsonc
[
  { "keys": "ctrl+shift+t", "command": "tab.new" },
  { "keys": "ctrl+shift+w", "command": "tab.close" },
  { "keys": "ctrl+shift+p", "command": "palette.open" },
  { "keys": "ctrl+shift+d", "command": "pane.split.vertical" },
  { "keys": "alt+arrowright", "command": "pane.focus.right" },
  { "keys": "ctrl+shift+f", "command": "search.open" },
  { "keys": "ctrl+shift+z", "command": "pane.zoom.toggle" }
]
```

Keys support chords (`ctrl+k ctrl+s`), context conditions (`when`), and map to
the command registry (the same ids the palette and plugins use).

## Theme engine ("CSS-like styling")

The UI exposes a constrained style layer: themes set CSS custom properties
(`--nova-bg`, `--nova-accent`, …) consumed by Tailwind utility classes and
component styles. Custom status bars and layouts are declarative JSON resolved
to these tokens — power without arbitrary CSS injection (which the security
model forbids in the webview).

## Precedence

`built-in defaults` → `config.json` → `workspace overrides` → `runtime (palette)`.
The effective config is recomputed and broadcast as `ConfigReloaded` on change.
