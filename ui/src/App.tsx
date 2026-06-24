import { useEffect, useMemo } from "react";
import { TitleBar } from "./components/TitleBar";
import { Tabs } from "./components/Tabs";
import { StatusBar } from "./components/StatusBar";
import { TerminalView } from "./components/TerminalView";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette";
import { closeSession, listThemes, onCoreEvent } from "./bridge";
import { dispatchFrame } from "./frames";
import { applyThemeToCss, useStore } from "./store";

export default function App() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const themes = useStore((s) => s.themes);

  // One-time: load themes, subscribe to core events, open the first tab.
  useEffect(() => {
    const s = useStore.getState();

    listThemes()
      .then((t) => {
        s.setThemes(t);
        const def = t.find((x) => x.id === "tokyo-night") ?? t[0];
        if (def) {
          applyThemeToCss(def);
          useStore.setState({ theme: def });
        }
      })
      .catch((e) => console.error("listThemes failed", e));

    const unlistenPromise = onCoreEvent((ev) => {
      switch (ev.event) {
        case "frame":
          dispatchFrame(ev);
          break;
        case "title_changed":
          useStore.getState().setTitle(ev.session, ev.title);
          break;
        case "cwd_changed":
          useStore.getState().setCwd(ev.session, ev.cwd);
          break;
        case "exited":
          useStore.getState().markExited(ev.session);
          break;
        default:
          break;
      }
    });

    if (s.tabs.length === 0) s.addTab();

    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, []);

  const newTab = () => useStore.getState().addTab();

  const closeTab = (id: string) => {
    const tab = useStore.getState().tabs.find((t) => t.id === id);
    if (tab?.sessionId) void closeSession(tab.sessionId);
    useStore.getState().closeTab(id);
  };

  // Global shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey) {
        const k = e.key.toUpperCase();
        if (k === "P") {
          e.preventDefault();
          useStore.getState().setPalette(!useStore.getState().paletteOpen);
        } else if (k === "T") {
          e.preventDefault();
          newTab();
        } else if (k === "W") {
          e.preventDefault();
          const id = useStore.getState().activeTabId;
          if (id) closeTab(id);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const actions: PaletteAction[] = useMemo(() => {
    const base: PaletteAction[] = [
      { id: "tab.new", title: "New Tab", hint: "Ctrl+Shift+T", run: newTab },
      {
        id: "tab.close",
        title: "Close Tab",
        hint: "Ctrl+Shift+W",
        run: () => {
          const id = useStore.getState().activeTabId;
          if (id) closeTab(id);
        },
      },
    ];
    const themeActions: PaletteAction[] = themes.map((t) => ({
      id: `theme.${t.id}`,
      title: `Theme: ${t.name}`,
      run: () => useStore.getState().setTheme(t.id),
    }));
    return [...base, ...themeActions];
  }, [themes]);

  return (
    <div className="flex h-full flex-col bg-nova-bg/0 text-nova-fg">
      <TitleBar />
      <Tabs onNewTab={newTab} onCloseTab={closeTab} />
      <div className="relative flex-1 overflow-hidden">
        {tabs.map((tab) => (
          <div key={tab.id} className="absolute inset-0">
            <TerminalView tab={tab} active={tab.id === activeTabId} />
          </div>
        ))}
        <CommandPalette actions={actions} />
      </div>
      <StatusBar />
    </div>
  );
}
