// Routes incoming frame diffs to the renderer registered for each session.
// Frame data never enters React state (it would cause re-render storms); only
// metadata (title/cwd/exit) goes through the store.

import type { FrameDiff, SessionId } from "./types";

type FrameHandler = (diff: FrameDiff) => void;

const handlers = new Map<SessionId, FrameHandler>();

export function registerFrameHandler(session: SessionId, handler: FrameHandler) {
  handlers.set(session, handler);
}

export function unregisterFrameHandler(session: SessionId) {
  handlers.delete(session);
}

export function dispatchFrame(diff: FrameDiff) {
  handlers.get(diff.session)?.(diff);
}
