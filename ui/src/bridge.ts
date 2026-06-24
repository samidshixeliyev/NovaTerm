// Thin typed wrapper over the Tauri command/event surface exposed by
// `nova-tauri`. Desktop only — these calls require the Tauri runtime.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CoreEvent, InputEvent, SessionId, Theme } from "./types";

export interface SpawnOptions {
  profileId?: string;
  cwd?: string;
  cols: number;
  rows: number;
}

export async function spawnSession(opts: SpawnOptions): Promise<SessionId> {
  return invoke<SessionId>("spawn_session", {
    profileId: opts.profileId ?? null,
    cwd: opts.cwd ?? null,
    cols: opts.cols,
    rows: opts.rows,
  });
}

export async function sendInput(session: SessionId, event: InputEvent): Promise<void> {
  await invoke("send_input", { session, event });
}

export async function resizeSession(
  session: SessionId,
  cols: number,
  rows: number,
  pixelWidth: number,
  pixelHeight: number,
): Promise<void> {
  await invoke("resize_session", { session, cols, rows, pixelWidth, pixelHeight });
}

export async function closeSession(session: SessionId): Promise<void> {
  await invoke("close_session", { session });
}

export async function requestFullFrame(session: SessionId): Promise<void> {
  await invoke("request_full_frame", { session });
}

export async function listThemes(): Promise<Theme[]> {
  return invoke<Theme[]>("list_themes");
}

/** Subscribe to the core event stream. Returns an unlisten function. */
export async function onCoreEvent(handler: (ev: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>("core-event", (e) => handler(e.payload));
}
