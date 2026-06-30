import type { ProviderDescriptor } from '@sb/types';

// The frontend provider seam: descriptors + capability flags drive what the UI
// shows. Kick is the only registered platform today; Twitch/YouTube/TikTok/Discord
// slot in here when their Rust ChatProvider impl exists.
export const KICK: ProviderDescriptor = {
  id: 'kick',
  displayName: 'Kick',
  auth: 'none', // anonymous Pusher read; OAuth becomes available in Settings later
  capabilities: { chat: true, voteCommands: true, userRoles: true, sendMessages: false },
};

export const providerRegistry: Record<string, ProviderDescriptor> = {
  kick: KICK,
};
