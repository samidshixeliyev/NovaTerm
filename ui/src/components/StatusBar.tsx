import { useStore } from "../store";

export function StatusBar() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const theme = useStore((s) => s.theme);
  const active = tabs.find((t) => t.id === activeTabId);

  return (
    <div className="flex h-6 items-center justify-between border-t border-nova-border/60 bg-nova-tabInactive/40 px-3 text-[11px] text-nova-fg/60">
      <div className="flex items-center gap-3">
        <span className="text-nova-accent">●</span>
        <span>{active?.cwd ?? "~"}</span>
        {active?.exited && <span className="text-red-400">exited</span>}
      </div>
      <div className="flex items-center gap-3">
        <span>{tabs.length} session{tabs.length === 1 ? "" : "s"}</span>
        <span>{theme?.name ?? "—"}</span>
        <span className="opacity-60">NovaTerm 0.1</span>
      </div>
    </div>
  );
}
