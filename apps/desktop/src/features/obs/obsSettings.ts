// OBS WebSocket config — renderer-only, persisted in localStorage (like the accent
// setting). No backend/contract involvement. NOTE: `password` is stored in plaintext;
// acceptable for a single-user local desktop app (see the security note in the PR).

export interface ObsScenes {
  battle: string;
  winner: string;
  intermission: string;
}

export interface ObsSettings {
  url: string;
  password: string;
  autoSwitch: boolean;
  scenes: ObsScenes;
  browserSourceName: string;
}

const KEY = 'sb.obs';

export const OBS_DEFAULTS: ObsSettings = {
  url: 'ws://127.0.0.1:4455',
  password: '',
  autoSwitch: false,
  scenes: { battle: '', winner: '', intermission: '' },
  browserSourceName: '',
};

export function loadObsSettings(): ObsSettings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return OBS_DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<ObsSettings>;
    return {
      ...OBS_DEFAULTS,
      ...parsed,
      scenes: { ...OBS_DEFAULTS.scenes, ...(parsed.scenes ?? {}) },
    };
  } catch {
    return OBS_DEFAULTS;
  }
}

export function saveObsSettings(settings: ObsSettings): void {
  localStorage.setItem(KEY, JSON.stringify(settings));
}
