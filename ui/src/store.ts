import { create } from "zustand";
import type { Profile, SessionId, Theme } from "./types";

export interface Tab {
  id: string;
  sessionId: SessionId | null;
  title: string;
  cwd: string | null;
  profileId?: string;
  pinned: boolean;
  exited: boolean;
}

interface AppState {
  tabs: Tab[];
  activeTabId: string | null;
  paletteOpen: boolean;
  themes: Theme[];
  theme: Theme | null;
  toast: string | null;
  profiles: Profile[];
  // settings
  settingsOpen: boolean;
  fontSize: number;
  cursorStyle: "bar" | "block" | "underline";
  defaultProfileId: string | null;

  addTab: (profileId?: string, title?: string) => string;
  setProfiles: (profiles: Profile[]) => void;
  setSettings: (open: boolean) => void;
  setFontSize: (n: number) => void;
  setCursorStyle: (s: "bar" | "block" | "underline") => void;
  setDefaultProfileId: (id: string) => void;
  attachSession: (tabId: string, sessionId: SessionId) => void;
  closeTab: (tabId: string) => void;
  setActive: (tabId: string) => void;
  setTitle: (sessionId: SessionId, title: string) => void;
  setCwd: (sessionId: SessionId, cwd: string) => void;
  markExited: (sessionId: SessionId) => void;
  togglePinned: (tabId: string) => void;
  setPalette: (open: boolean) => void;
  setThemes: (themes: Theme[]) => void;
  setTheme: (id: string) => void;
  setToast: (msg: string | null) => void;
}

let tabCounter = 0;

export const useStore = create<AppState>((set, get) => ({
  tabs: [],
  activeTabId: null,
  paletteOpen: false,
  themes: [],
  theme: null,
  toast: null,
  profiles: [],
  settingsOpen: false,
  fontSize: Number(localStorage.getItem("nova.fontSize")) || 14,
  cursorStyle: (localStorage.getItem("nova.cursorStyle") as "bar" | "block" | "underline") || "bar",
  defaultProfileId: localStorage.getItem("nova.defaultProfile"),

  addTab: (profileId, title) => {
    const id = `tab-${++tabCounter}`;
    const tab: Tab = { id, sessionId: null, title: title ?? "Shell", cwd: null, profileId, pinned: false, exited: false };
    set((s) => ({ tabs: [...s.tabs, tab], activeTabId: id }));
    return id;
  },

  setProfiles: (profiles) => set({ profiles }),

  attachSession: (tabId, sessionId) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === tabId ? { ...t, sessionId } : t)) })),

  closeTab: (tabId) =>
    set((s) => {
      const tabs = s.tabs.filter((t) => t.id !== tabId);
      let activeTabId = s.activeTabId;
      if (activeTabId === tabId) {
        activeTabId = tabs.length ? tabs[Math.max(0, tabs.length - 1)].id : null;
      }
      return { tabs, activeTabId };
    }),

  setActive: (tabId) => set({ activeTabId: tabId }),

  setTitle: (sessionId, title) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.sessionId === sessionId ? { ...t, title } : t)) })),

  setCwd: (sessionId, cwd) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.sessionId === sessionId ? { ...t, cwd } : t)) })),

  markExited: (sessionId) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.sessionId === sessionId ? { ...t, exited: true } : t)) })),

  togglePinned: (tabId) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === tabId ? { ...t, pinned: !t.pinned } : t)) })),

  setPalette: (open) => set({ paletteOpen: open }),

  setThemes: (themes) => set({ themes }),

  setTheme: (id) => {
    const theme = get().themes.find((t) => t.id === id) ?? null;
    if (theme) applyThemeToCss(theme);
    set({ theme });
  },

  setToast: (toast) => set({ toast }),

  setSettings: (settingsOpen) => set({ settingsOpen }),
  setFontSize: (fontSize) => {
    localStorage.setItem("nova.fontSize", String(fontSize));
    set({ fontSize });
  },
  setCursorStyle: (cursorStyle) => {
    localStorage.setItem("nova.cursorStyle", cursorStyle);
    set({ cursorStyle });
  },
  setDefaultProfileId: (id) => {
    localStorage.setItem("nova.defaultProfile", id);
    set({ defaultProfileId: id });
  },
}));

/** Push a theme's colors into the CSS custom properties consumed by the UI. */
export function applyThemeToCss(theme: Theme) {
  const r = document.documentElement.style;
  r.setProperty("--nova-bg", theme.ui.bg);
  r.setProperty("--nova-fg", theme.ui.fg);
  r.setProperty("--nova-accent", theme.ui.accent);
  r.setProperty("--nova-border", theme.ui.border);
  r.setProperty("--nova-tab-active", theme.ui.tab_active);
  r.setProperty("--nova-tab-inactive", theme.ui.tab_inactive);
}
