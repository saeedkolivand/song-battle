// Shared domain DTOs — the integration contract between the Rust backend and both
// frontends. The Rust serde structs are the source of truth and MUST serialize to
// these exact shapes (`#[serde(rename_all = "camelCase")]`). Overlay (WS) and
// dashboard (Tauri events) both consume `Snapshot`.

export type Source = 'youtube' | 'spotify' | 'soundcloud';

export interface Song {
  id: string;
  title: string;
  artist?: string;
  thumbnail?: string;
  durationSec?: number;
  source: Source;
  sourceUrl: string;
  submitter?: string;
  metadata?: Record<string, unknown>;
}

export interface MediaMetadata {
  title: string;
  artist?: string;
  thumbnail?: string;
  durationSec?: number;
  source: Source;
  sourceUrl: string;
}

export type ConnectionState =
  'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'error';

export interface ChatUser {
  userId: string;
  username: string;
  displayName: string;
  isMod: boolean;
  isSub: boolean;
  isVip: boolean;
}

export interface ChatMessage {
  id: string;
  user: ChatUser;
  text: string;
  ts: number;
}

export type ProviderId = 'kick' | 'twitch' | 'youtube' | 'tiktok' | 'discord';

export interface ProviderDescriptor {
  id: ProviderId;
  displayName: string;
  auth: 'none' | 'oauth' | 'token';
  capabilities: {
    chat: boolean;
    voteCommands: boolean;
    userRoles: boolean;
    sendMessages: boolean;
  };
}

// ── Live state ────────────────────────────────────────────────────────────────
// One snapshot is broadcast to the overlay (WS) and the dashboard (Tauri events),
// carrying a monotonic `seq` so clients can drop stale frames.

export type MatchState = 'pending' | 'active' | 'done';
export type BattleStatus = 'idle' | 'running' | 'finished';

// Tournament structure. 'single' = single-elim (bestOf 1); 'double' = double-elim
// (winners/losers/grand brackets, bestOf 1); 'bo3' = single-elim, each match best-of-3.
export type BattleMode = 'single' | 'double' | 'bo3';

// Which sub-bracket a match belongs to. 'main' for single-elim/bo3; the rest for double-elim.
export type MatchGroup = 'main' | 'winners' | 'losers' | 'grand';

export interface TimerView {
  durationSec: number;
  remainingSec: number;
  running: boolean;
}

export interface MatchView {
  id: string;
  round: number; // 1-based
  a: Song | null; // null = bye / not-yet-decided slot
  b: Song | null;
  votesA: number;
  votesB: number;
  pctA: number; // 0..100, integer
  pctB: number;
  total: number;
  state: MatchState;
  winner: 'a' | 'b' | null;
  timer: TimerView | null; // present while active
  group: MatchGroup; // double-elim sub-bracket; 'main' otherwise
  bestOf: number; // 1, or 3 for bo3 mode
  winsA: number; // games won in the series (a)
  winsB: number; // games won in the series (b)
}

export interface BattleView {
  id: string;
  title: string;
  description: string;
  theme: string;
  mode: BattleMode;
  status: BattleStatus;
  round: number; // current round, 1-based
  totalRounds: number;
  currentMatch: MatchView | null;
  bracket: MatchView[]; // every match, for bracket-progress rendering
  winner: Song | null; // overall winner once finished
  songs: Song[]; // full roster (dashboard Songs page; overlay ignores it)
  songCount: number;
}

export interface KickView {
  state: ConnectionState;
  channel: string | null;
}

// Official Kick API (OAuth) auth state — separate from the unofficial (Pusher)
// `KickView` above. `subscriptionActive` is unused until K2 wires the webhook.
export interface KickOfficialStatus {
  authorized: boolean;
  subscriptionActive: boolean;
}

export interface Snapshot {
  seq: number;
  battle: BattleView | null;
  kick: KickView;
  anonymous: boolean; // when true, overlay/recent-votes hide voter identities
}

// Vote-command parsing lives in Rust; this is the normalized result the tally uses.
export type VoteChoice = 'a' | 'b';

// Persisted app settings (Rust get_settings/set_*).
export interface Settings {
  anonymous: boolean;
  defaultTimerSec: number;
  chatSubmissions: boolean; // allow viewers to add songs via !submit <url> (lobby only)
}

// Summary row for the saved-tournaments list (list_battles command).
export interface SavedBattle {
  id: string;
  title: string;
  theme: string;
  status: BattleStatus;
  songCount: number;
  updatedAt: number; // epoch ms
}
