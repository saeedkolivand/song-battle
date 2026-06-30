import { useEffect } from 'react';
import { useBattleStore } from '../../stores/battle';
import { useDevStore } from './devStore';

/**
 * Feed the dev recorder from the battle store. Mounted once at the app shell (like
 * useObsAutoSwitch) so it records every snapshot regardless of the open page.
 * Subscribing to an external store is the valid use of an effect.
 */
export function useDevRecorder(): void {
  useEffect(() => {
    const { record } = useDevStore.getState();

    // Capture whatever is already in the store (recorder may mount after the first frame).
    const initial = useBattleStore.getState().snapshot;
    if (initial) record(initial);

    return useBattleStore.subscribe((state, prev) => {
      if (state.snapshot && state.snapshot !== prev.snapshot) {
        useDevStore.getState().record(state.snapshot);
      }
    });
  }, []);
}
