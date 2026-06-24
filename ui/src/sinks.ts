// Routes raw PTY output to the xterm.js instance for each session. Bytes never
// pass through React state — only the xterm write callback receives them.

import type { SessionId } from "./types";

type OutputSink = (bytes: Uint8Array) => void;

const sinks = new Map<SessionId, OutputSink>();

export function registerOutput(session: SessionId, sink: OutputSink) {
  sinks.set(session, sink);
}

export function unregisterOutput(session: SessionId) {
  sinks.delete(session);
}

export function dispatchOutput(session: SessionId, base64: string) {
  const sink = sinks.get(session);
  if (sink) sink(base64ToBytes(base64));
}

/** Decode base64 → bytes (PTY output is shipped base64-encoded). */
export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}
