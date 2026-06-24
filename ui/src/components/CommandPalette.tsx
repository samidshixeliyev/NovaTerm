import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../store";

export interface PaletteAction {
  id: string;
  title: string;
  hint?: string;
  run: () => void;
}

export function CommandPalette({ actions }: { actions: PaletteAction[] }) {
  const open = useStore((s) => s.paletteOpen);
  const setPalette = useStore((s) => s.setPalette);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setIndex(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.toLowerCase().trim();
    if (!q) return actions;
    return actions.filter((a) => a.title.toLowerCase().includes(q));
  }, [actions, query]);

  if (!open) return null;

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") setPalette(false);
    else if (e.key === "ArrowDown") setIndex((i) => Math.min(i + 1, filtered.length - 1));
    else if (e.key === "ArrowUp") setIndex((i) => Math.max(i - 1, 0));
    else if (e.key === "Enter") {
      const action = filtered[index];
      if (action) {
        setPalette(false);
        action.run();
      }
    }
  };

  return (
    <div
      className="absolute inset-0 z-50 flex items-start justify-center bg-black/30 pt-24"
      onClick={() => setPalette(false)}
    >
      <div
        className="w-[34rem] max-w-[90vw] overflow-hidden rounded-xl border border-nova-border bg-nova-tabActive shadow-2xl animate-scale-in"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={onKeyDown}
          placeholder="Type a command…"
          className="w-full bg-transparent px-4 py-3 text-sm outline-none placeholder:text-nova-fg/40"
        />
        <div className="max-h-80 overflow-y-auto border-t border-nova-border/60">
          {filtered.length === 0 && <div className="px-4 py-3 text-sm text-nova-fg/40">No matching commands</div>}
          {filtered.map((a, i) => (
            <div
              key={a.id}
              onMouseEnter={() => setIndex(i)}
              onClick={() => {
                setPalette(false);
                a.run();
              }}
              className={`flex cursor-default items-center justify-between px-4 py-2 text-sm ${
                i === index ? "bg-nova-accent/20" : ""
              }`}
            >
              <span>{a.title}</span>
              {a.hint && <span className="text-xs text-nova-fg/40">{a.hint}</span>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
