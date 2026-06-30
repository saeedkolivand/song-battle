import type { Snapshot } from '@sb/types';

export type SnapshotHandler = (s: Snapshot) => void;
export type OpenHandler = (open: boolean) => void;

/**
 * Reconnecting overlay WebSocket client. The overlay is a dumb projection of
 * server snapshots, so this just (re)connects and forwards parsed messages.
 * Returns a disposer.
 *
 * ponytail: exponential backoff capped at 5s; no seq-gap recovery yet — the
 * server resends a full snapshot on connect (Phase 1), so a reconnect self-heals.
 */
export function connectOverlay(
  url: string,
  onSnapshot: SnapshotHandler,
  onOpen?: OpenHandler,
): () => void {
  let ws: WebSocket | null = null;
  let closed = false;
  let backoff = 500;

  const open = () => {
    ws = new WebSocket(url);
    ws.onopen = () => {
      backoff = 500;
      onOpen?.(true);
    };
    ws.onmessage = (e) => {
      try {
        onSnapshot(JSON.parse(e.data as string) as Snapshot);
      } catch {
        /* ignore malformed frame */
      }
    };
    ws.onclose = () => {
      onOpen?.(false);
      if (!closed) {
        backoff = Math.min(backoff * 2, 5000);
        setTimeout(open, backoff);
      }
    };
    ws.onerror = () => ws?.close();
  };

  open();
  return () => {
    closed = true;
    ws?.close();
  };
}
