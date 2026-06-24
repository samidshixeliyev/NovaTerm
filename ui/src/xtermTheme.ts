// Map a NovaTerm theme to an xterm.js ITheme.

import type { ITheme } from "@xterm/xterm";
import type { Theme } from "./types";

export function toXtermTheme(t: Theme): ITheme {
  const a = t.ansi;
  return {
    background: t.ui.bg,
    foreground: t.ui.fg,
    cursor: t.cursor,
    cursorAccent: t.ui.bg,
    selectionBackground: t.selection,
    black: a.black,
    red: a.red,
    green: a.green,
    yellow: a.yellow,
    blue: a.blue,
    magenta: a.magenta,
    cyan: a.cyan,
    white: a.white,
    brightBlack: a.bright_black,
    brightRed: a.bright_red,
    brightGreen: a.bright_green,
    brightYellow: a.bright_yellow,
    brightBlue: a.bright_blue,
    brightMagenta: a.bright_magenta,
    brightCyan: a.bright_cyan,
    brightWhite: a.bright_white,
  };
}
