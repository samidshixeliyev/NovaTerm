import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import "@xterm/xterm/css/xterm.css";

import { closeSession, resizeSession, spawnSession, writeText } from "../bridge";
import { registerOutput, unregisterOutput } from "../sinks";
import { useStore, type Tab } from "../store";
import { toXtermTheme } from "../xtermTheme";
import type { SessionId } from "../types";

const FONT_FAMILY =
  "'CaskaydiaCove Nerd Font Mono', 'Cascadia Code', 'JetBrains Mono', Consolas, monospace";

export function TerminalView({ tab, active }: { tab: Tab; active: boolean }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<SessionId | null>(null);
  const theme = useStore((s) => s.theme);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const term = new Terminal({
      fontFamily: FONT_FAMILY,
      fontSize: 14,
      lineHeight: 1.15,
      letterSpacing: 0,
      cursorBlink: true,
      cursorStyle: "bar",
      scrollback: 100_000,
      allowProposedApi: true,
      macOptionIsMeta: true,
      theme: theme ? toXtermTheme(theme) : undefined,
    });
    termRef.current = term;

    const fit = new FitAddon();
    fitRef.current = fit;
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.loadAddon(new WebLinksAddon());

    term.open(host);

    // Prefer the GPU (WebGL) renderer; fall back silently to DOM if unavailable.
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      term.loadAddon(webgl);
    } catch {
      /* DOM renderer fallback */
    }

    const doFit = () => {
      try {
        fit.fit();
      } catch {
        /* host not measurable yet */
      }
    };
    doFit();
    // Re-fit once the bundled font is ready (metrics change after load).
    if (document.fonts?.ready) void document.fonts.ready.then(doFit);

    let disposed = false;
    spawnSession({ profileId: tab.profileId, cols: term.cols, rows: term.rows })
      .then((sid) => {
        if (disposed) {
          void closeSession(sid);
          return;
        }
        sessionRef.current = sid;
        useStore.getState().attachSession(tab.id, sid);
        registerOutput(sid, (bytes) => term.write(bytes));
      })
      .catch((err) => {
        useStore.getState().setToast(`Failed to start shell: ${err}`);
        term.write(`\r\n\x1b[31mFailed to start shell: ${err}\x1b[0m\r\n`);
      });

    const dataSub = term.onData((data) => {
      const sid = sessionRef.current;
      if (sid) void writeText(sid, data);
    });
    const resizeSub = term.onResize(({ cols, rows }) => {
      const sid = sessionRef.current;
      if (sid) void resizeSession(sid, cols, rows, host.clientWidth, host.clientHeight);
    });
    const titleSub = term.onTitleChange((title) => {
      const sid = sessionRef.current;
      if (sid && title) useStore.getState().setTitle(sid, title);
    });

    const ro = new ResizeObserver(() => doFit());
    ro.observe(host);

    return () => {
      disposed = true;
      ro.disconnect();
      dataSub.dispose();
      resizeSub.dispose();
      titleSub.dispose();
      const sid = sessionRef.current;
      if (sid) unregisterOutput(sid);
      term.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id]);

  // Live theme switching without recreating the session.
  useEffect(() => {
    if (theme && termRef.current) termRef.current.options.theme = toXtermTheme(theme);
  }, [theme]);

  // Focus + refit when this tab becomes active.
  useEffect(() => {
    if (active && termRef.current) {
      termRef.current.focus();
      requestAnimationFrame(() => {
        try {
          fitRef.current?.fit();
        } catch {
          /* ignore */
        }
      });
    }
  }, [active]);

  return (
    <div
      className="h-full w-full"
      style={{ padding: 8, display: active ? "block" : "none" }}
    >
      <div ref={hostRef} className="h-full w-full" />
    </div>
  );
}
