import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

function Btn({ onClick, label, danger }: { onClick: () => void; label: string; danger?: boolean }) {
  return (
    <button
      onClick={onClick}
      className={`no-drag grid h-8 w-12 place-items-center text-xs transition-colors hover:bg-white/10 ${
        danger ? "hover:bg-red-500/80" : ""
      }`}
    >
      {label}
    </button>
  );
}

export function TitleBar() {
  return (
    <div className="drag-region flex h-9 items-center justify-between border-b border-nova-border/60 bg-nova-tabInactive/40 select-none">
      <div className="flex items-center gap-2 px-3">
        <div className="h-3.5 w-3.5 rounded-sm bg-nova-accent" />
        <span className="text-xs font-semibold tracking-wide opacity-80">NovaTerm</span>
      </div>
      <div className="flex">
        <Btn onClick={() => void appWindow.minimize()} label="—" />
        <Btn onClick={() => void appWindow.toggleMaximize()} label="▢" />
        <Btn onClick={() => void appWindow.close()} label="✕" danger />
      </div>
    </div>
  );
}
