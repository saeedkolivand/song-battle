// Shared domain DTOs. These mirror the Rust serde structs (single source of truth
// lives in src-tauri); keep field names in sync across the FFI boundary.

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
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'error';

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

// State broadcast to overlay (WS) and dashboard (Tauri events). Both consume the
// SAME snapshot so they can never disagree. Phase 0 carries only a heartbeat; the
// full battle/vote/timer/bracket fields land in Phase 1.
export interface Snapshot {
  seq: number;
  counter: number;
}
