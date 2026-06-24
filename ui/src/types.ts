// Mirror of `nova-protocol` wire types (desktop/Tauri IPC only).

export type SessionId = string;

/** Packed RGBA color `0xRRGGBBAA`; 0 means "use theme default". */
export type Color = number;

export const ATTR = {
  BOLD: 1 << 0,
  ITALIC: 1 << 1,
  UNDERLINE: 1 << 2,
  STRIKETHROUGH: 1 << 3,
  INVERSE: 1 << 4,
  DIM: 1 << 5,
  BLINK: 1 << 6,
  HIDDEN: 1 << 7,
  WIDE: 1 << 8,
  WIDE_SPACER: 1 << 9,
  HYPERLINK: 1 << 10,
} as const;

export interface Cell {
  ch: string;
  fg: Color;
  bg: Color;
  attrs: number;
}

export type CursorShape = "block" | "bar" | "underline";

export interface CursorState {
  row: number;
  col: number;
  shape: CursorShape;
  visible: boolean;
  blink: boolean;
}

export interface RowRun {
  row: number;
  col: number;
  cells: Cell[];
}

export interface ScrollRegion {
  top: number;
  bottom: number;
  delta: number;
}

export interface FrameDiff {
  session: SessionId;
  seq: number;
  cols: number;
  rows: number;
  full: boolean;
  scroll: ScrollRegion | null;
  runs: RowRun[];
  cursor: CursorState;
  scrollback_len: number;
}

// Tagged union mirror of `CoreEvent` (serde `#[serde(tag = "event")]`).
export type CoreEvent =
  | { event: "spawned"; session: SessionId; pid: number }
  | { event: "output"; session: SessionId; base64: string }
  | { event: "frame"; session: SessionId; seq: number; cols: number; rows: number; full: boolean; scroll: ScrollRegion | null; runs: RowRun[]; cursor: CursorState; scrollback_len: number }
  | { event: "title_changed"; session: SessionId; title: string }
  | { event: "cwd_changed"; session: SessionId; cwd: string }
  | { event: "bell"; session: SessionId }
  | { event: "exited"; session: SessionId; code: number }
  | { event: "error"; session: SessionId | null; message: string };

export interface KeyModifiers {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
}

// Tagged union mirror of `InputEvent` (serde `#[serde(tag = "type")]`).
export type InputEvent =
  | { type: "key"; key: string; mods: KeyModifiers; text: string | null }
  | { type: "paste"; text: string }
  | { type: "resize"; cols: number; rows: number; pixel_width: number; pixel_height: number };

export interface ThemeColorsUi {
  bg: string;
  fg: string;
  accent: string;
  border: string;
  tab_active: string;
  tab_inactive: string;
}

export interface Profile {
  id: string;
  name: string;
  shell: string;
  args: string[];
  icon: string | null;
  color: string | null;
}

export interface Theme {
  id: string;
  name: string;
  builtin: boolean;
  ui: ThemeColorsUi;
  ansi: Record<string, string>;
  cursor: string;
  selection: string;
}
