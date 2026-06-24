import type { ReactNode } from "react";
import { relaunchElevated } from "../bridge";
import { useStore } from "../store";

const CURSORS: Array<"bar" | "block" | "underline"> = ["bar", "block", "underline"];

export function Settings() {
  const open = useStore((s) => s.settingsOpen);
  const setSettings = useStore((s) => s.setSettings);
  const themes = useStore((s) => s.themes);
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const profiles = useStore((s) => s.profiles);
  const fontSize = useStore((s) => s.fontSize);
  const setFontSize = useStore((s) => s.setFontSize);
  const cursorStyle = useStore((s) => s.cursorStyle);
  const setCursorStyle = useStore((s) => s.setCursorStyle);
  const defaultProfileId = useStore((s) => s.defaultProfileId);
  const setDefaultProfileId = useStore((s) => s.setDefaultProfileId);
  const setToast = useStore((s) => s.setToast);

  if (!open) return null;

  return (
    <div
      className="absolute inset-0 z-[60] flex items-center justify-center bg-black/40 backdrop-blur-sm"
      onClick={() => setSettings(false)}
    >
      <div
        className="flex max-h-[80vh] w-[40rem] max-w-[92vw] flex-col overflow-hidden rounded-2xl border border-nova-border bg-nova-tabActive shadow-2xl animate-scale-in"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-nova-border/60 px-5 py-3">
          <h2 className="text-base font-semibold">Settings</h2>
          <button
            onClick={() => setSettings(false)}
            className="grid h-7 w-7 place-items-center rounded-md text-nova-fg/60 hover:bg-white/10 hover:text-nova-fg"
          >
            ✕
          </button>
        </div>

        <div className="flex-1 space-y-6 overflow-y-auto px-5 py-4">
          <Section title="Appearance">
            <Row label="Theme">
              <select
                value={theme?.id ?? ""}
                onChange={(e) => setTheme(e.target.value)}
                className="rounded-md border border-nova-border bg-nova-bg px-2 py-1.5 text-sm outline-none"
              >
                {themes.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
            </Row>
            <Row label={`Font size — ${fontSize}px`}>
              <input
                type="range"
                min={9}
                max={28}
                value={fontSize}
                onChange={(e) => setFontSize(Number(e.target.value))}
                className="w-48 accent-nova-accent"
              />
            </Row>
            <Row label="Cursor">
              <div className="flex gap-1">
                {CURSORS.map((c) => (
                  <button
                    key={c}
                    onClick={() => setCursorStyle(c)}
                    className={`rounded-md px-3 py-1 text-sm capitalize ${
                      cursorStyle === c ? "bg-nova-accent/30 text-nova-fg" : "bg-nova-bg text-nova-fg/60 hover:bg-white/5"
                    }`}
                  >
                    {c}
                  </button>
                ))}
              </div>
            </Row>
          </Section>

          <Section title="Shell">
            <Row label="Default shell (new tabs)">
              <select
                value={defaultProfileId ?? ""}
                onChange={(e) => setDefaultProfileId(e.target.value)}
                className="rounded-md border border-nova-border bg-nova-bg px-2 py-1.5 text-sm outline-none"
              >
                <option value="">(system default)</option>
                {profiles.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </Row>
          </Section>

          <Section title="Privileges">
            <Row label="Administrator">
              <button
                onClick={() => {
                  relaunchElevated().catch((e) => setToast(`Run as admin failed: ${e}`));
                }}
                className="rounded-md border border-nova-accent/40 bg-nova-accent/15 px-3 py-1.5 text-sm hover:bg-nova-accent/25"
              >
                Restart as Administrator
              </button>
            </Row>
            <p className="text-xs text-nova-fg/40">
              Windows can't elevate a single tab — this relaunches NovaTerm elevated (UAC prompt), so all
              tabs run as admin.
            </p>
          </Section>
        </div>

        <div className="flex items-center justify-between border-t border-nova-border/60 px-5 py-3 text-xs text-nova-fg/40">
          <span>NovaTerm</span>
          <span>Settings are saved locally</span>
        </div>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-nova-fg/40">{title}</h3>
      <div className="space-y-3 rounded-xl border border-nova-border/50 bg-nova-bg/40 p-3">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-sm text-nova-fg/80">{label}</span>
      {children}
    </div>
  );
}
