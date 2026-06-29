import { useState } from "react";
import { useStore } from "../store";

/** Last path segment of a shell command, e.g. ".../bin/bash.exe" → "bash.exe". */
function shellBase(shell: string): string {
  const parts = shell.split(/[\\/]/);
  return parts[parts.length - 1] || shell;
}

export function Tabs({
  onNewTab,
  onCloseTab,
}: {
  onNewTab: (profileId?: string, name?: string) => void;
  onCloseTab: (id: string) => void;
}) {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const setActive = useStore((s) => s.setActive);
  const togglePinned = useStore((s) => s.togglePinned);
  const profiles = useStore((s) => s.profiles);
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <div className="drag-region flex h-10 items-center gap-1 border-b border-nova-border/60 px-2">
      {/* Scrolling tab strip */}
      <div className="no-drag flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {tabs.map((tab) => {
          const isActive = tab.id === activeTabId;
          return (
            <div
              key={tab.id}
              onClick={() => setActive(tab.id)}
              onDoubleClick={() => togglePinned(tab.id)}
              className={`group flex h-8 shrink-0 cursor-default items-center gap-2 rounded-lg px-3 text-sm transition-all animate-fade-in ${
                isActive
                  ? "bg-nova-tabActive text-nova-fg shadow-sm"
                  : "bg-nova-tabInactive/40 text-nova-fg/60 hover:bg-nova-tabInactive"
              } ${tab.pinned ? "max-w-[3rem]" : "max-w-[14rem]"}`}
              title={tab.cwd ?? tab.title}
            >
              <span className={`h-2 w-2 rounded-full ${tab.exited ? "bg-red-400" : "bg-nova-accent"}`} />
              {!tab.pinned && <span className="truncate">{tab.title}</span>}
              {!tab.pinned && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onCloseTab(tab.id);
                  }}
                  className="ml-1 hidden h-4 w-4 place-items-center rounded text-xs hover:bg-white/20 group-hover:grid"
                >
                  ✕
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* New-tab split button — OUTSIDE the scroll area so its menu isn't clipped */}
      <div className="no-drag relative flex shrink-0 items-center">
        <button
          onClick={() => onNewTab()}
          className="grid h-8 w-8 place-items-center rounded-l-lg text-lg text-nova-fg/70 hover:bg-nova-tabInactive hover:text-nova-fg"
          title="New tab (Ctrl+Shift+T)"
        >
          +
        </button>
        <button
          onClick={() => setMenuOpen((v) => !v)}
          className="grid h-8 w-5 place-items-center rounded-r-lg text-[10px] text-nova-fg/60 hover:bg-nova-tabInactive hover:text-nova-fg"
          title="Choose a shell"
        >
          ▾
        </button>
        {menuOpen && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setMenuOpen(false)} />
            <div className="absolute right-0 top-9 z-50 min-w-[13rem] overflow-hidden rounded-lg border border-nova-border bg-nova-tabActive py-1 shadow-2xl animate-scale-in">
              <div className="px-3 py-1 text-[10px] uppercase tracking-wide text-nova-fg/40">
                Available terminals
              </div>
              {profiles.length === 0 && (
                <div className="px-3 py-2 text-xs text-nova-fg/40">Detecting installed shells…</div>
              )}
              {profiles.map((p) => (
                <button
                  key={p.id}
                  onClick={() => {
                    setMenuOpen(false);
                    onNewTab(p.id, p.name);
                  }}
                  className="flex w-full items-center gap-3 px-3 py-1.5 text-left text-sm hover:bg-nova-accent/20"
                >
                  <span
                    className="grid h-5 w-5 shrink-0 place-items-center rounded text-[11px]"
                    style={{ background: (p.color ?? "#3a3a3a") + "33", color: p.color ?? undefined }}
                  >
                    {p.icon ?? "›"}
                  </span>
                  <span className="min-w-0 flex-1 truncate">{p.name}</span>
                  <span className="shrink-0 text-[11px] text-nova-fg/35">{shellBase(p.shell)}</span>
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
