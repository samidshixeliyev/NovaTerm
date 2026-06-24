// NovaTerm 2D-canvas terminal renderer.
//
// This is the universal fallback backend described in docs/RENDERING.md (the
// WebGPU/WebGL backends use the same cell-buffer + diff-apply contract). It is
// dirty-region aware: the core ships only changed rows, and we repaint only
// those rows plus the cursor overlay, skipping frames entirely when idle.

import type { Cell, CursorState, FrameDiff } from "../types";
import { ATTR } from "../types";

export interface RendererTheme {
  bg: string;
  fg: string;
  cursor: string;
}

const EMPTY_CELL: Cell = { ch: " ", fg: 0, bg: 0, attrs: 0 };

export interface CellMetrics {
  cellWidth: number;
  cellHeight: number;
}

export class TerminalRenderer {
  private ctx: CanvasRenderingContext2D;
  private dpr = Math.max(1, Math.floor(window.devicePixelRatio || 1));

  private fontFamily: string;
  private fontSize: number;
  private lineHeight: number;
  private metrics: CellMetrics = { cellWidth: 8, cellHeight: 16 };

  private cols = 0;
  private rows = 0;
  private buffer: Cell[][] = [];
  private dirty: Set<number> = new Set();
  private cursor: CursorState = { row: 0, col: 0, shape: "block", visible: true, blink: true };

  private theme: RendererTheme;
  private rafHandle = 0;
  private needsPaint = true;
  private cursorBlinkOn = true;
  private lastBlink = 0;

  constructor(
    private canvas: HTMLCanvasElement,
    opts: { fontFamily: string; fontSize: number; lineHeight: number; theme: RendererTheme },
  ) {
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) throw new Error("2D canvas context unavailable");
    this.ctx = ctx;
    this.fontFamily = opts.fontFamily;
    this.fontSize = opts.fontSize;
    this.lineHeight = opts.lineHeight;
    this.theme = opts.theme;
    this.measure();
    this.loop = this.loop.bind(this);
    this.rafHandle = requestAnimationFrame(this.loop);
  }

  dispose() {
    cancelAnimationFrame(this.rafHandle);
  }

  setTheme(theme: RendererTheme) {
    this.theme = theme;
    this.markAllDirty();
  }

  setFont(fontFamily: string, fontSize: number, lineHeight: number) {
    this.fontFamily = fontFamily;
    this.fontSize = fontSize;
    this.lineHeight = lineHeight;
    this.measure();
    this.markAllDirty();
  }

  getMetrics(): CellMetrics {
    return this.metrics;
  }

  /** Grid dimensions that fit the given pixel size. */
  gridForPixels(pixelWidth: number, pixelHeight: number): { cols: number; rows: number } {
    return {
      cols: Math.max(1, Math.floor(pixelWidth / this.metrics.cellWidth)),
      rows: Math.max(1, Math.floor(pixelHeight / this.metrics.cellHeight)),
    };
  }

  /** Resize the backing canvas to the CSS pixel size (HiDPI-aware). */
  resizeCanvas(pixelWidth: number, pixelHeight: number) {
    this.dpr = Math.max(1, Math.floor(window.devicePixelRatio || 1));
    this.canvas.width = Math.floor(pixelWidth * this.dpr);
    this.canvas.height = Math.floor(pixelHeight * this.dpr);
    this.canvas.style.width = `${pixelWidth}px`;
    this.canvas.style.height = `${pixelHeight}px`;
    this.ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    this.applyFontToCtx();
    this.markAllDirty();
  }

  /** Apply a frame diff from the core to the local cell buffer. */
  applyFrame(diff: FrameDiff) {
    if (diff.full || diff.cols !== this.cols || diff.rows !== this.rows) {
      this.allocate(diff.cols, diff.rows);
    }

    if (diff.scroll) {
      this.applyScroll(diff.scroll.top, diff.scroll.bottom, diff.scroll.delta);
    }

    for (const run of diff.runs) {
      const row = this.buffer[run.row];
      if (!row) continue;
      for (let i = 0; i < run.cells.length; i++) {
        const col = run.col + i;
        if (col < this.cols) row[col] = run.cells[i];
      }
      this.dirty.add(run.row);
    }

    this.cursor = diff.cursor;
    this.needsPaint = true;
  }

  // --- internals -------------------------------------------------------

  private measure() {
    this.applyFontToCtx();
    const w = this.ctx.measureText("M").width || this.fontSize * 0.6;
    this.metrics = {
      cellWidth: Math.max(1, Math.round(w)),
      cellHeight: Math.max(1, Math.round(this.fontSize * this.lineHeight)),
    };
  }

  private applyFontToCtx() {
    this.ctx.font = `${this.fontSize}px ${this.fontFamily}`;
    this.ctx.textBaseline = "top";
  }

  private allocate(cols: number, rows: number) {
    this.cols = cols;
    this.rows = rows;
    this.buffer = Array.from({ length: rows }, () =>
      Array.from({ length: cols }, () => EMPTY_CELL),
    );
    this.markAllDirty();
  }

  private applyScroll(top: number, bottom: number, delta: number) {
    if (delta === 0) return;
    const region = this.buffer.slice(top, bottom + 1);
    const blankRow = () => Array.from({ length: this.cols }, () => EMPTY_CELL);
    if (delta > 0) {
      region.splice(0, Math.min(delta, region.length));
      while (region.length < bottom - top + 1) region.push(blankRow());
    } else {
      const n = Math.min(-delta, region.length);
      for (let i = 0; i < n; i++) region.unshift(blankRow());
      region.length = bottom - top + 1;
    }
    for (let i = 0; i < region.length; i++) {
      this.buffer[top + i] = region[i];
      this.dirty.add(top + i);
    }
  }

  private markAllDirty() {
    this.dirty.clear();
    for (let r = 0; r < this.rows; r++) this.dirty.add(r);
    this.needsPaint = true;
  }

  private loop(ts: number) {
    // Cursor blink at ~1.6 Hz; only forces a paint of the cursor row.
    if (this.cursor.blink && ts - this.lastBlink > 600) {
      this.lastBlink = ts;
      this.cursorBlinkOn = !this.cursorBlinkOn;
      this.dirty.add(this.cursor.row);
      this.needsPaint = true;
    }

    if (this.needsPaint && this.dirty.size > 0) {
      this.paint();
    }
    this.needsPaint = false;
    this.rafHandle = requestAnimationFrame(this.loop);
  }

  private paint() {
    const { cellWidth: cw, cellHeight: ch } = this.metrics;
    const ctx = this.ctx;

    for (const row of this.dirty) {
      const cells = this.buffer[row];
      if (!cells) continue;
      const y = row * ch;

      // Clear the row to the theme background.
      ctx.fillStyle = this.theme.bg;
      ctx.fillRect(0, y, this.cols * cw, ch);

      for (let col = 0; col < this.cols; col++) {
        const cell = cells[col];
        if (cell.attrs & ATTR.WIDE_SPACER) continue;
        const inverse = (cell.attrs & ATTR.INVERSE) !== 0;
        const fg = inverse ? this.bgColor(cell) : this.fgColor(cell);
        const bg = inverse ? this.fgColor(cell) : this.bgColor(cell);
        const width = cell.attrs & ATTR.WIDE ? 2 : 1;
        const x = col * cw;

        if (bg !== this.theme.bg) {
          ctx.fillStyle = bg;
          ctx.fillRect(x, y, cw * width, ch);
        }

        if (cell.ch !== " " && !(cell.attrs & ATTR.HIDDEN)) {
          const bold = (cell.attrs & ATTR.BOLD) !== 0;
          const italic = (cell.attrs & ATTR.ITALIC) !== 0;
          ctx.font = `${italic ? "italic " : ""}${bold ? "bold " : ""}${this.fontSize}px ${this.fontFamily}`;
          ctx.globalAlpha = cell.attrs & ATTR.DIM ? 0.6 : 1;
          ctx.fillStyle = fg;
          ctx.fillText(cell.ch, x, y + (ch - this.fontSize) / 2);
          ctx.globalAlpha = 1;

          if (cell.attrs & ATTR.UNDERLINE) {
            ctx.fillRect(x, y + ch - 2, cw * width, 1);
          }
          if (cell.attrs & ATTR.STRIKETHROUGH) {
            ctx.fillRect(x, y + ch / 2, cw * width, 1);
          }
        }
      }
    }

    this.dirty.clear();
    this.drawCursor();
  }

  private drawCursor() {
    if (!this.cursor.visible) return;
    if (this.cursor.blink && !this.cursorBlinkOn) return;
    const { cellWidth: cw, cellHeight: ch } = this.metrics;
    const x = this.cursor.col * cw;
    const y = this.cursor.row * ch;
    this.ctx.fillStyle = this.theme.cursor;
    switch (this.cursor.shape) {
      case "bar":
        this.ctx.fillRect(x, y, 2, ch);
        break;
      case "underline":
        this.ctx.fillRect(x, y + ch - 2, cw, 2);
        break;
      default: {
        // Block cursor: fill and redraw the glyph under it in the bg color.
        this.ctx.globalAlpha = 0.8;
        this.ctx.fillRect(x, y, cw, ch);
        this.ctx.globalAlpha = 1;
        const cell = this.buffer[this.cursor.row]?.[this.cursor.col];
        if (cell && cell.ch !== " ") {
          this.ctx.fillStyle = this.theme.bg;
          this.ctx.fillText(cell.ch, x, y + (ch - this.fontSize) / 2);
        }
      }
    }
  }

  private fgColor(cell: Cell): string {
    return cell.fg === 0 ? this.theme.fg : rgbaToCss(cell.fg);
  }
  private bgColor(cell: Cell): string {
    return cell.bg === 0 ? this.theme.bg : rgbaToCss(cell.bg);
  }
}

/** Convert packed `0xRRGGBBAA` to a CSS color. */
export function rgbaToCss(color: number): string {
  const r = (color >>> 24) & 0xff;
  const g = (color >>> 16) & 0xff;
  const b = (color >>> 8) & 0xff;
  const a = color & 0xff;
  return a === 0xff ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${(a / 255).toFixed(3)})`;
}
