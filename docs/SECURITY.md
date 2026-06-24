# NovaTerm — Security Model

## Threat model

NovaTerm runs untrusted *output* (shell programs can emit arbitrary escape
sequences) and optionally untrusted *plugins*. Primary risks:

1. Malicious escape sequences (e.g. clipboard hijack, title injection, DECRQSS
   probes, OSC abuse).
2. Plugins exfiltrating data or executing arbitrary code.
3. Hyperlink / path spoofing leading to unintended execution.
4. Settings-sync data leakage.

## Escape-sequence hardening

- **Allow-listed OSC handlers.** Clipboard writes via OSC 52 are **off by
  default**; when enabled they are size-capped and require the window to be
  focused. Reads are never granted to the shell.
- **Title sanitization.** Window/tab titles strip control chars and are length
  capped; no escape passthrough into native window APIs.
- **Bracketed paste enforced.** Multi-line pastes are wrapped and a confirmation
  is shown when the paste contains newlines or control characters (anti
  paste-and-run).
- **Hyperlinks (OSC 8)** are shown with their *real* target on hover; clicking
  http(s) opens the browser; `file:`/exec-like schemes require explicit confirm.

## Plugin sandbox

- Plugins run in a separate **plugin host process**, never in core or webview.
- WASM plugins: Wasmtime with no WASI ambient capabilities; CPU fuel-metered,
  memory-capped, no direct syscalls. JS plugins: no Node/`fs`/`net` by default.
- **Capability-based permissions**, deny-by-default, user-approved at install,
  revocable. Every brokered call is permission-checked at the host boundary
  (see [PLUGINS.md](PLUGINS.md)).
- Plugins cannot read other plugins' storage or sessions they weren't granted.

## IPC boundary (UI ↔ core)

- Tauri commands are explicitly allow-listed in the capability config; the
  webview cannot call arbitrary Rust.
- All command inputs are validated/typed via `nova-protocol`; ids are opaque
  UUIDs (no path or handle leakage to the UI).
- CSP locks the webview to local assets; no remote code execution in the UI.

## Secrets & storage

- The SQLite DB and config live under `%APPDATA%/NovaTerm` with user-only ACLs.
- SSH manager credentials are stored via Windows Credential Manager / DPAPI,
  never in plaintext JSON or the DB.
- Environment presets may reference secrets by name (resolved from the OS vault),
  not store raw values.

## Settings sync

- End-to-end encrypted: a sync key derived from a user passphrase (Argon2id)
  encrypts payloads (XChaCha20-Poly1305) before they leave the device.
- The sync server (if used) stores only ciphertext + version metadata.
- Conflict resolution is last-writer-wins per key with per-device ids; secrets
  are *excluded* from sync.

## Updates

- The built-in updater verifies a signature (minisign/ed25519) over the release
  manifest and artifact before applying. Downgrade protection via monotonic
  version check. Update channel stored in `kv`.

## Telemetry / analytics

- Local-only by default. Any cloud analytics is opt-in, aggregated, and contains
  no command text or output — only counters and timings.
