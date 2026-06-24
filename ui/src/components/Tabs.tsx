import { useStore } from "../store";

export function Tabs({ onNewTab, onCloseTab }: { onNewTab: () => void; onCloseTab: (id: string) => void }) {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const setActive = useStore((s) => s.setActive);
  const togglePinned = useStore((s) => s.togglePinned);

  return (
    <div className="drag-region flex h-10 items-center gap-1 border-b border-nova-border/60 px-2">
      <div className="no-drag flex flex-1 items-center gap-1 overflow-x-auto">
        {tabs.map((tab) => {
          const isActive = tab.id === activeTabId;
          return (
            <div
              key={tab.id}
              onClick={() => setActive(tab.id)}
              onDoubleClick={() => togglePinned(tab.id)}
              className={`group flex h-8 cursor-default items-center gap-2 rounded-lg px-3 text-sm transition-all animate-fade-in ${
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
        <button
          onClick={onNewTab}
          className="grid h-8 w-8 place-items-center rounded-lg text-lg text-nova-fg/60 hover:bg-nova-tabInactive hover:text-nova-fg"
          title="New tab (Ctrl+Shift+T)"
        >
          +
        </button>
      </div>
    </div>
  );
}
