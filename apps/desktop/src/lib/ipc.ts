import { invoke } from '@tauri-apps/api/core';
import type { Snapshot, SavedBattle, Settings, BattleMode, KickOfficialStatus } from '@sb/types';

// Canonical type lives in @sb/types; alias kept for existing call sites.
export type AppSettings = Settings;

// True only inside the Tauri app window (it injects __TAURI_INTERNALS__). In a
// plain browser there is no IPC, so invoke/listen would throw — guard UI on this.
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

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
  reorderSongs: (orderedIds: string[]) => invoke<void>('reorder_songs', { orderedIds }),

  generateBracket: (mode: BattleMode) => invoke<void>('generate_bracket', { mode }),
  startMatch: () => invoke<void>('start_match'),
  resetVotes: () => invoke<void>('reset_votes'),
  skipMatch: () => invoke<void>('skip_match'),
  setTimer: (durationSec: number) => invoke<void>('set_timer', { durationSec }),

  connectKick: (channel: string, chatroomId?: number) =>
    invoke<void>('connect_kick', { channel, chatroomId: chatroomId ?? null }),
  disconnectKick: () => invoke<void>('disconnect_kick'),

  // Official Kick API (OAuth) — alongside the unofficial (Pusher) path above.
  kickOauthStart: (clientId: string, clientSecret: string) =>
    invoke<string>('kick_oauth_start', { clientId, clientSecret }),
  kickOfficialStatus: () => invoke<KickOfficialStatus>('kick_official_status'),
  kickOfficialDisconnect: () => invoke<void>('kick_official_disconnect'),

  exportJson: () => invoke<string>('export_json'),
  importJson: (json: string) => invoke<void>('import_json', { json }),

  // Saved tournaments (Phase 2).
  listBattles: () => invoke<SavedBattle[]>('list_battles'),
  loadBattle: (id: string) => invoke<void>('load_battle', { id }),
  deleteBattle: (id: string) => invoke<void>('delete_battle', { id }),

  // Persisted settings (Phase 2).
  getSettings: () => invoke<AppSettings>('get_settings'),
  setAnonymous: (anonymous: boolean) => invoke<void>('set_anonymous', { anonymous }),
  setDefaultTimer: (sec: number) => invoke<void>('set_default_timer', { sec }),
  setChatSubmissions: (enabled: boolean) => invoke<void>('set_chat_submissions', { enabled }),

  overlayUrl: () => invoke<string>('overlay_url'),
  openOverlayWindow: () => invoke<void>('open_overlay_window'),
  ping: () => invoke<string>('ping'),
};
