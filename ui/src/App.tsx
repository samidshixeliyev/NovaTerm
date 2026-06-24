import { useEffect, useMemo } from "react";
import { TitleBar } from "./components/TitleBar";
import { Tabs } from "./components/Tabs";
import { StatusBar } from "./components/StatusBar";
import { TerminalView } from "./components/TerminalView";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette";
import { Settings } from "./components/Settings";
import { closeSession, defaultProfile, listProfiles, listThemes, onCoreEvent, relaunchElevated } from "./bridge";
import { dispatchOutput } from "./sinks";
import { applyThemeToCss, useStore } from "./store";

export default function App() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const themes = useStore((s) => s.themes);
  const profiles = useStore((s) => s.profiles);

  // One-time: load themes + profiles, subscribe to core events, open first tab.
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

    // Load shell profiles, then open the first tab with the default profile.
    Promise.all([listProfiles(), defaultProfile()])
      .then(([profs, def]) => {
        s.setProfiles(profs);
        if (useStore.getState().tabs.length === 0) {
          const p = profs.find((x) => x.id === def) ?? profs[0];
          s.addTab(p?.id, p?.name);
        }
      })
      .catch(() => {
        if (useStore.getState().tabs.length === 0) s.addTab();
      });

    const unlistenPromise = onCoreEvent((ev) => {
      switch (ev.event) {
        case "output":
          dispatchOutput(ev.session, ev.base64);
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
        case "error":
          if (ev.message) useStore.getState().setToast(ev.message);
          break;
        default:
          break;
      }
    });

    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, []);

  const newTab = (profileId?: string, name?: string) => {
    const st = useStore.getState();
    if (!profileId && st.defaultProfileId) {
      const p = st.profiles.find((x) => x.id === st.defaultProfileId);
      return st.addTab(p?.id, p?.name);
    }
    return st.addTab(profileId, name);
  };

  const closeTab = (id: string) => {
    const tab = useStore.getState().tabs.find((t) => t.id === id);
    if (tab?.sessionId) void closeSession(tab.sessionId);
    useStore.getState().closeTab(id);
  };

  // Global shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && e.key === ",") {
        e.preventDefault();
        useStore.getState().setSettings(true);
        return;
      }
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
      { id: "tab.new", title: "New Tab", hint: "Ctrl+Shift+T", run: () => newTab() },
      {
        id: "tab.close",
        title: "Close Tab",
        hint: "Ctrl+Shift+W",
        run: () => {
          const id = useStore.getState().activeTabId;
          if (id) closeTab(id);
        },
      },
      { id: "settings.open", title: "Settings", hint: "Ctrl+,", run: () => useStore.getState().setSettings(true) },
      {
        id: "admin",
        title: "Run as Administrator",
        run: () => relaunchElevated().catch((e) => useStore.getState().setToast(`Run as admin failed: ${e}`)),
      },
    ];
    const profileActions: PaletteAction[] = profiles.map((p) => ({
      id: `tab.new.${p.id}`,
      title: `New Tab: ${p.name}`,
      hint: p.shell,
      run: () => newTab(p.id, p.name),
    }));
    const themeActions: PaletteAction[] = themes.map((t) => ({
      id: `theme.${t.id}`,
      title: `Theme: ${t.name}`,
      run: () => useStore.getState().setTheme(t.id),
    }));
    return [...base, ...profileActions, ...themeActions];
  }, [themes, profiles]);

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
        <Settings />
        <Toast />
      </div>
      <StatusBar />
    </div>
  );
}

function Toast() {
  const toast = useStore((s) => s.toast);
  const setToast = useStore((s) => s.setToast);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 6000);
    return () => clearTimeout(t);
  }, [toast, setToast]);
  if (!toast) return null;
  return (
    <div
      className="absolute bottom-4 left-1/2 z-50 -translate-x-1/2 animate-fade-in cursor-pointer rounded-lg border border-red-500/40 bg-red-500/15 px-4 py-2 text-sm text-nova-fg shadow-xl backdrop-blur"
      onClick={() => setToast(null)}
    >
      {toast}
    </div>
  );
}
