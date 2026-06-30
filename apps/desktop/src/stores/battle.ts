import { create } from 'zustand';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { Snapshot } from '@sb/types';
import { ipc } from '../lib/ipc';

// Read-model cache only — never the source of truth. The backend owns battle
// state; this just mirrors the latest snapshot for rendering. A monotonic `seq`
// lets us drop out-of-order frames.
interface BattleState {
  snapshot: Snapshot | null;
  live: boolean;
  error: string | null;
  apply: (s: Snapshot) => void;
}

/**
 * Pure seq-drop reducer: returns the snapshot to keep. A newer `seq` replaces the
 * previous one; a stale OR duplicate (equal) `seq` is dropped — returns `prev`
 * unchanged so the store can skip a redundant render.
 */
export function applySnapshot(prev: Snapshot | null, next: Snapshot): Snapshot {
  if (prev && next.seq <= prev.seq) return prev;
  return next;
}

export const useBattleStore = create<BattleState>((set, get) => ({
  snapshot: null,
  live: false,
  error: null,
  apply: (s) => {
    const cur = get().snapshot;
    const kept = applySnapshot(cur, s);
    if (kept === cur) return; // stale/duplicate frame — no state change
    set({ snapshot: kept, live: true, error: null });
  },
}));

/**
 * Sync the store with the backend: seed from `get_snapshot`, then subscribe to
 * the `snapshot` event. Subscribing to an external system is the textbook valid
 * use of an effect — call this once from App and dispose on unmount.
 */
export async function startSnapshotStream(): Promise<UnlistenFn> {
  const { apply } = useBattleStore.getState();
  try {
    apply(await ipc.getSnapshot());
  } catch (e) {
    useBattleStore.setState({ error: e instanceof Error ? e.message : String(e) });
  }
  return listen<Snapshot>('snapshot', (ev) => apply(ev.payload));
}
