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

export const useBattleStore = create<BattleState>((set, get) => ({
  snapshot: null,
  live: false,
  error: null,
  apply: (s) => {
    const cur = get().snapshot;
    if (cur && s.seq < cur.seq) return; // stale frame — ignore
    set({ snapshot: s, live: true, error: null });
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
