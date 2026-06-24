# NovaTerm — Rendering Engine

## Goal

Paint a terminal grid at up to 240 Hz with zero-lag scrolling, repainting only
what changed, using a cached glyph atlas. Three backends, auto-selected:

1. **WebGPU** (preferred) — instanced quad rendering, one draw call per frame.
2. **WebGL2** — fallback for older GPUs / driver issues.
3. **2D Canvas** — universal fallback, still dirty-region aware.

## Pipeline

```
FrameDiff (from core)
   │  apply to CellBuffer (renderer-owned, source of truth for pixels)
   ▼
Damage tracker  ──▶ list of dirty rows / regions
   ▼
Glyph shaper    ──▶ (codepoint, style) → atlas slot   [cache hit ≈ 99%]
   ▼
Instance builder──▶ per-cell quad: pos, uv, fg, bg, flags   (typed array)
   ▼
GPU draw        ──▶ 1 instanced draw for glyphs + 1 for backgrounds
   ▼
Overlays        ──▶ cursor, selection, search highlights, links (separate pass)
```

## Glyph atlas

- A single GPU texture (default 2048×2048, grows by page). Each distinct
  `(glyph, weight, italic, font-size, subpixel-bin)` occupies one slot.
- Rasterization: the browser's text metrics + an offscreen canvas raster, or
  (native path, future) DirectWrite via the Rust side feeding bitmaps.
- LRU eviction when full; CJK/emoji handled as wide (2-column) glyphs; color
  emoji stored as RGBA, ASCII as R8 coverage (subpixel/grayscale AA).
- The atlas is **shared across all tabs/panes** — a primary memory win.

## Cell model (renderer side)

```ts
// 12 bytes packed per cell in a Uint8Array-backed structure-of-arrays
interface Cell {
  glyph: u32;   // codepoint (or ligature cluster id)
  fg: u32;      // rgba
  bg: u32;      // rgba
  flags: u16;   // bold,italic,underline,strike,inverse,wide,wide-spacer,link
}
```

Structure-of-arrays (separate typed arrays for glyph/fg/bg/flags) keeps the
instance upload a few `set()` calls — no per-cell object churn, GC-free.

## Dirty-region rendering

The core sends only changed cells, but the renderer also tracks its own damage
(cursor move, selection, blink) so overlay-only changes never re-upload the grid.
Per frame we union dirty rects; if < 30% of the screen is dirty we do partial
scissored draws, otherwise a full redraw (cheaper than many tiny ones).

## Scrolling

Scrollback lives in the renderer as a ring of rows. Scrolling changes a
*viewport offset* only — no IPC, no re-layout. New output appended at the bottom
shifts the offset if the user is pinned to the bottom; otherwise the viewport
stays put (scroll-lock). Smooth scroll interpolates the offset over a few frames.

## Frame scheduling

- Driven by `requestAnimationFrame`; target cadence matches the monitor (up to
  240 Hz). The core's frame-tick (configurable 4–16 ms) is independent.
- If no diffs and no overlay damage: **skip the frame entirely** (0% GPU at
  idle — key to low power and the memory/idle targets).
- Adaptive: under sustained heavy output we drop to coalesced batches to keep
  the UI responsive (never render faster than we can present).

## Ligatures & shaping

Programming ligatures (Fira Code, Cascadia) are detected per contiguous run of
same-style cells; a shaped cluster maps to a single wide atlas entry. Falls back
to per-glyph when the font lacks the feature.

## Performance budget (1080p, 200×50 grid)

| Stage | Budget |
|---|---|
| Diff apply | < 0.3 ms |
| Instance build | < 0.5 ms |
| GPU draw | < 1.5 ms |
| Total frame | < 4 ms (leaves headroom for 240 Hz) |
