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
  const fontSize = useStore((s) => s.fontSize);
  const cursorStyle = useStore((s) => s.cursorStyle);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const s0 = useStore.getState();

    const term = new Terminal({
      fontFamily: FONT_FAMILY,
      fontSize: s0.fontSize,
      lineHeight: 1.15,
      letterSpacing: 0,
      cursorBlink: true,
      cursorStyle: s0.cursorStyle,
      scrollback: 100_000,
      allowProposedApi: true,
      macOptionIsMeta: true,
      windowsPty: { backend: "conpty" },
      theme: s0.theme ? toXtermTheme(s0.theme) : undefined,
    });
    termRef.current = term;

    // Ctrl+C copies the selection (else passes through as interrupt); Ctrl+V pastes.
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown" || !e.ctrlKey || e.altKey) return true;
      const k = e.key.toLowerCase();
      if (k === "c" && term.hasSelection()) {
        navigator.clipboard.writeText(term.getSelection()).catch(() => {});
        return false;
      }
      if (k === "v") {
        navigator.clipboard
          .readText()
          .then((t) => {
            const sid = sessionRef.current;
            if (sid && t) void writeText(sid, t);
          })
          .catch(() => {});
        return false;
      }
      return true;
    });

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

  // Live font-size / cursor-style changes from Settings.
  useEffect(() => {
    const t = termRef.current;
    if (!t) return;
    t.options.fontSize = fontSize;
    t.options.cursorStyle = cursorStyle;
    requestAnimationFrame(() => {
      try {
        fitRef.current?.fit();
      } catch {
        /* ignore */
      }
    });
  }, [fontSize, cursorStyle]);

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

  const restart = () => {
    const st = useStore.getState();
    if (tab.sessionId) void closeSession(tab.sessionId);
    st.addTab(tab.profileId, tab.title);
    st.closeTab(tab.id);
  };

  return (
    <div
      className="relative h-full w-full"
      style={{ padding: 8, display: active ? "block" : "none" }}
    >
      <div ref={hostRef} className="h-full w-full" />
      {tab.exited && (
        <div className="pointer-events-none absolute inset-x-0 bottom-3 flex justify-center">
          <div className="pointer-events-auto flex items-center gap-3 rounded-lg border border-nova-border bg-nova-tabActive/95 px-4 py-2 text-sm shadow-xl backdrop-blur animate-fade-in">
            <span className="h-2 w-2 rounded-full bg-red-400" />
            <span className="text-nova-fg/80">Process exited</span>
            <button
              onClick={restart}
              className="rounded-md border border-nova-accent/40 bg-nova-accent/15 px-2.5 py-1 text-xs font-medium hover:bg-nova-accent/25"
            >
              Restart
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
