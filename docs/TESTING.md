# NovaTerm — Testing Strategy & CI/CD

## Test pyramid

| Layer | Scope | Tools |
|---|---|---|
| Unit | grid mutations, VTE handling, diffing, config merge, storage queries | `cargo test`, `vitest` |
| Property | parser invariants (no panics on arbitrary bytes), diff round-trip | `proptest` |
| Golden | feed recorded escape-sequence streams, assert grid snapshot | custom harness in `nova-terminal/tests` |
| Integration | spawn real ConPTY shell, drive commands, assert output | `cargo test --features integration` (Windows-only) |
| UI component | renderer math, stores, palette filtering | `vitest` + `@testing-library/react` |
| E2E | launch app, type, screenshot diff | Tauri + WebDriver (`tauri-driver`) |
| Bench | startup, frame time, throughput (lines/sec), memory | `criterion`, custom perf harness |

## Key invariants under test

- The VTE parser never panics on any byte sequence (fuzzed).
- `apply(diff)` on the renderer cell buffer reproduces the core grid exactly
  (recording == playback path — covered by replay tests).
- Resize preserves content per ConPTY reflow rules.
- Config merge is deterministic and last-good on parse error.
- Scrollback ring + disk spill round-trips losslessly.

## Performance gates (fail CI on regression)

- Cold start > 30 ms → fail.
- Idle RSS > 80 MB → fail.
- p99 frame time > 8 ms at 120 Hz under stream load → fail.
- `cat` of a 1M-line file completes within budget without UI stall.

## CI/CD pipeline (GitHub Actions)

```yaml
name: ci
on: [push, pull_request]
jobs:
  rust:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - run: cargo test --workspace --features integration
  ui:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: cd ui && npm ci
      - run: cd ui && npm run lint && npm run typecheck && npm run test
  bench:
    runs-on: windows-latest
    needs: [rust]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo bench --workspace -- --save-baseline pr
      # compare against main baseline; fail on >5% regression on tracked metrics
  release:
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: windows-latest
    needs: [rust, ui]
    steps:
      - uses: actions/checkout@v4
      - run: cd ui && npm ci && npm run tauri build
      - name: Sign installer
        run: ./scripts/sign.ps1   # Authenticode + minisign manifest
      - uses: softprops/action-gh-release@v2
        with: { files: "ui/src-tauri/target/release/bundle/**/*" }
```

## Branch policy

- All PRs: fmt + clippy (`-D warnings`) + tests must pass.
- `main` is always releasable; tags `vX.Y.Z` trigger signed release builds.
- Perf job comments the benchmark delta on the PR.
