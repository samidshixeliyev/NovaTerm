# NovaTerm — Plugin SDK & Architecture

## Goals

Sandboxed, hot-reloadable plugins with a capability-based permission system,
two authoring languages (Rust → WASM, JavaScript), and a marketplace-ready
manifest format.

## Execution model

| Plugin kind | Runtime | Isolation |
|---|---|---|
| Rust / WASM | Wasmtime (WASI subset) | Memory-isolated VM, fuel-metered CPU, no ambient syscalls |
| JavaScript  | Sandboxed JS engine (deno_core/QuickJS) | No Node, no `fs`/`net` unless granted |

Plugins **never** run in the UI webview or the core's address space. They run in
a dedicated **plugin host** process; the host brokers all capability calls.

```
core ──┐
       │  capability RPC (typed, permission-checked)
plugin host ── Wasmtime VM (rust plugin)
            └─ JS sandbox  (js plugin)
```

## Manifest (`plugin.json`)

```json
{
  "id": "com.novaterm.docker",
  "name": "Docker",
  "version": "1.0.0",
  "entry": "plugin.wasm",
  "kind": "wasm",
  "permissions": [
    "terminal:read",
    "terminal:write",
    "ui:statusbar",
    "ui:panel",
    "process:spawn:docker",
    "net:unix:/var/run/docker.sock"
  ],
  "contributes": {
    "commands": [{ "id": "docker.ps", "title": "Docker: List Containers" }],
    "statusbar": [{ "id": "docker.status", "align": "right" }],
    "panels": [{ "id": "docker.panel", "title": "Containers", "icon": "whale" }]
  }
}
```

## Permission system

Capability-based, deny-by-default. Each permission is a typed grant the user
approves at install time (and can revoke). Examples:

- `terminal:read` / `terminal:write` — observe/inject in the *active* session only.
- `process:spawn:<exe>` — spawn a specific allow-listed executable.
- `net:tcp:<host:port>` / `net:unix:<path>` — scoped network egress.
- `ui:statusbar` / `ui:panel` / `ui:command` — UI contribution surfaces.
- `storage:plugin` — private key/value store namespaced to the plugin.

The host enforces every call; a plugin cannot widen its own grants at runtime.

## SDK surface (Rust)

```rust
use novaterm_sdk::*;

#[novaterm::plugin]
struct Docker;

impl Plugin for Docker {
    fn activate(&mut self, ctx: &mut Context) -> Result<()> {
        ctx.register_command("docker.ps", |ctx| {
            let out = ctx.spawn("docker", &["ps", "--format", "json"])?;
            ctx.panel("docker.panel").set_rows(parse(out));
            Ok(())
        })?;
        ctx.statusbar("docker.status").set_text("🐳 ready");
        Ok(())
    }
}
```

## SDK surface (JavaScript)

```js
export default {
  activate(ctx) {
    ctx.commands.register("docker.ps", async () => {
      const out = await ctx.process.spawn("docker", ["ps", "--format", "json"]);
      ctx.panel("docker.panel").setRows(JSON.parse(out));
    });
    ctx.statusbar("docker.status").setText("🐳 ready");
  },
};
```

## Hot reload

The host watches the plugin file; on change it: drains in-flight calls,
`deactivate()`s the old instance, swaps the VM, `activate()`s the new one — all
without restarting NovaTerm. Plugin state can opt into `storage:plugin` to
survive reloads.

## Bundled example plugins

Docker, Kubernetes, AWS, Azure, PostgreSQL, MySQL, Redis, GitHub — each ships as
a reference implementation in `plugins/` (post-MVP) demonstrating panels,
status-bar items, and scoped process/network permissions.

## Marketplace

Plugins are distributed as signed `.novaplugin` bundles (manifest + wasm/js +
assets, zip). The registry stores signatures; the host verifies the signature
and shows the requested permission set before install.
