import { invoke } from '@tauri-apps/api/core';
import type { Snapshot } from '@sb/types';

// Typed wrappers over the Rust command surface. Command *names* are snake_case
// (the Rust side); Tauri converts camelCase JS arg keys to snake_case params.
// Mutations resolve to void — the resulting state arrives via the `snapshot`
// event (see stores/battle.ts), so the store stays the single read-model.
export const ipc = {
  getSnapshot: () => invoke<Snapshot>('get_snapshot'),

  createBattle: (title: string, description: string, theme: string) =>
    invoke<void>('create_battle', { title, description, theme }),

  importSong: (url: string, submitter?: string) =>
    invoke<void>('import_song', { url, submitter: submitter ?? null }),
  removeSong: (songId: string) => invoke<void>('remove_song', { songId }),
  shuffleSongs: () => invoke<void>('shuffle_songs'),

  generateBracket: () => invoke<void>('generate_bracket'),
  startMatch: () => invoke<void>('start_match'),
  resetVotes: () => invoke<void>('reset_votes'),
  skipMatch: () => invoke<void>('skip_match'),
  setTimer: (durationSec: number) => invoke<void>('set_timer', { durationSec }),

  connectKick: (channel: string) => invoke<void>('connect_kick', { channel }),
  disconnectKick: () => invoke<void>('disconnect_kick'),

  exportJson: () => invoke<string>('export_json'),
  importJson: (json: string) => invoke<void>('import_json', { json }),

  overlayUrl: () => invoke<string>('overlay_url'),
  ping: () => invoke<string>('ping'),
};
