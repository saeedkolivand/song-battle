import { useEffect, useRef } from 'react';
import { create } from 'zustand';
import type { ConnectionState, Snapshot } from '@sb/types';
import { log } from '@sb/shared';
import { useBattleStore } from '../../stores/battle';
import { obsClient } from './obsClient';
import { loadObsSettings, saveObsSettings, type ObsScenes, type ObsSettings } from './obsSettings';

interface ObsState {
  state: ConnectionState;
  error: string | null;
  settings: ObsSettings;
  scenes: string[]; // discovered scene names (populated when connected)
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  switchScene: (name: string) => Promise<void>;
  setBrowserSourceUrl: (inputName: string, url: string) => Promise<void>;
  refreshScenes: () => Promise<void>;
  patchSettings: (patch: Partial<Omit<ObsSettings, 'scenes'>>) => void;
  setScene: (which: keyof ObsScenes, value: string) => void;
}

export const useObsStore = create<ObsState>((set, get) => {
  // Mirror client status into the store (wired once, at store creation).
  obsClient.onStatus((state, error) => {
    set({ state, error: error ?? null });
    if (state === 'connected') void get().refreshScenes();
    else set({ scenes: [] });
  });

  return {
    state: 'disconnected',
    error: null,
    settings: loadObsSettings(),
    scenes: [],

    connect: async () => {
      const { url, password } = get().settings;
      await obsClient.connect(url, password);
    },
    disconnect: async () => {
      await obsClient.disconnect();
    },
    switchScene: async (name) => {
      await obsClient.switchScene(name);
    },
    setBrowserSourceUrl: async (inputName, url) => {
      await obsClient.setBrowserSourceUrl(inputName, url);
    },
    refreshScenes: async () => {
      try {
        set({ scenes: await obsClient.listScenes() });
      } catch {
        /* non-fatal: scene dropdown just stays empty */
      }
    },

    patchSettings: (patch) => {
      const next = { ...get().settings, ...patch };
      saveObsSettings(next);
      set({ settings: next });
    },
    setScene: (which, value) => {
      const next = { ...get().settings, scenes: { ...get().settings.scenes, [which]: value } };
      saveObsSettings(next);
      set({ settings: next });
    },
  };
});

// Map the live battle snapshot to the OBS scene that should be on program.
function targetScene(snapshot: Snapshot | null, scenes: ObsScenes): string | null {
  const battle = snapshot?.battle ?? null;
  if (battle && battle.status === 'finished') return scenes.winner || null;
  if (battle?.currentMatch?.state === 'active') return scenes.battle || null;
  return scenes.intermission || null;
}

/**
 * Auto-switch OBS scenes from battle state. Mounted once at the app shell so it runs
 * on every page. Acts only when connected, auto-switch is on, the target scene name is
 * set, and it changed — debounced ~300ms and de-duped against the last sent scene.
 */
export function useObsAutoSwitch(): void {
  const snapshot = useBattleStore((s) => s.snapshot);
  const connected = useObsStore((s) => s.state === 'connected');
  const autoSwitch = useObsStore((s) => s.settings.autoSwitch);
  const scenes = useObsStore((s) => s.settings.scenes);
  const switchScene = useObsStore((s) => s.switchScene);

  const target = targetScene(snapshot, scenes);
  const lastSent = useRef<string | null>(null);

  // Re-send after a reconnect even if the target is unchanged.
  useEffect(() => {
    if (!connected) lastSent.current = null;
  }, [connected]);

  useEffect(() => {
    if (!connected || !autoSwitch || !target || target === lastSent.current) return;
    const id = setTimeout(() => {
      lastSent.current = target;
      void switchScene(target).catch((e: unknown) => log.error('OBS auto-switch failed', e));
    }, 300);
    return () => clearTimeout(id);
  }, [connected, autoSwitch, target, switchScene]);
}
