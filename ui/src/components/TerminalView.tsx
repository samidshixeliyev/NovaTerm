import { useEffect, useRef } from "react";
import { TerminalRenderer } from "../renderer/TerminalRenderer";
import { registerFrameHandler, unregisterFrameHandler } from "../frames";
import { resizeSession, sendInput, spawnSession } from "../bridge";
import { useStore, type Tab } from "../store";
import type { InputEvent, KeyModifiers, SessionId } from "../types";

const PADDING = 10;
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "CapsLock", "Dead", "Process"]);

function mods(e: React.KeyboardEvent): KeyModifiers {
  return { ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey };
}

export function TerminalView({ tab, active }: { tab: Tab; active: boolean }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<TerminalRenderer | null>(null);
  const sessionRef = useRef<SessionId | null>(null);
  const theme = useStore((s) => s.theme);
  const attachSession = useStore((s) => s.attachSession);

  // Set up the renderer + PTY session once, when the component mounts.
  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;

    const t = theme;
    const renderer = new TerminalRenderer(canvas, {
      fontFamily: "Cascadia Code, Consolas, monospace",
      fontSize: 14,
      lineHeight: 1.3,
      theme: {
        bg: t?.ui.bg ?? "#1a1b26",
        fg: t?.ui.fg ?? "#c0caf5",
        cursor: t?.cursor ?? "#c0caf5",
      },
    });
    rendererRef.current = renderer;

    const w = container.clientWidth - PADDING * 2;
    const h = container.clientHeight - PADDING * 2;
    renderer.resizeCanvas(w, h);
    const { cols, rows } = renderer.gridForPixels(w, h);

    let disposed = false;
    spawnSession({ profileId: tab.profileId, cols, rows })
      .then((sid) => {
        if (disposed) return;
        sessionRef.current = sid;
        attachSession(tab.id, sid);
        registerFrameHandler(sid, (diff) => renderer.applyFrame(diff));
      })
      .catch((err) => console.error("spawn failed", err));

    const ro = new ResizeObserver(() => {
      const cw = container.clientWidth - PADDING * 2;
      const chh = container.clientHeight - PADDING * 2;
      renderer.resizeCanvas(cw, chh);
      const grid = renderer.gridForPixels(cw, chh);
      const sid = sessionRef.current;
      if (sid) void resizeSession(sid, grid.cols, grid.rows, cw, chh);
    });
    ro.observe(container);

    return () => {
      disposed = true;
      ro.disconnect();
      const sid = sessionRef.current;
      if (sid) unregisterFrameHandler(sid);
      renderer.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id]);

  // React to theme changes without recreating the session.
  useEffect(() => {
    if (theme && rendererRef.current) {
      rendererRef.current.setTheme({ bg: theme.ui.bg, fg: theme.ui.fg, cursor: theme.cursor });
    }
  }, [theme]);

  // Focus the active terminal so it receives keystrokes.
  useEffect(() => {
    if (active) containerRef.current?.focus();
  }, [active]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const sid = sessionRef.current;
    if (!sid || MODIFIER_KEYS.has(e.key)) return;
    // Let global app shortcuts (palette, new tab) bubble.
    if (e.ctrlKey && e.shiftKey && ["P", "T", "W"].includes(e.key.toUpperCase())) return;

    e.preventDefault();
    const event: InputEvent = { type: "key", key: e.key, mods: mods(e), text: null };
    void sendInput(sid, event);
  };

  const onPaste = (e: React.ClipboardEvent) => {
    const sid = sessionRef.current;
    if (!sid) return;
    e.preventDefault();
    const text = e.clipboardData.getData("text");
    if (text) void sendInput(sid, { type: "paste", text });
  };

  return (
    <div
      ref={containerRef}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPaste={onPaste}
      className="h-full w-full outline-none"
      style={{ padding: PADDING, display: active ? "block" : "none" }}
    >
      <canvas ref={canvasRef} className="block" />
      {tab.exited && (
        <div className="pointer-events-none absolute bottom-10 left-1/2 -translate-x-1/2 rounded-md bg-black/60 px-3 py-1 text-sm">
          Process exited — press Ctrl+Shift+W to close
        </div>
      )}
    </div>
  );
}
