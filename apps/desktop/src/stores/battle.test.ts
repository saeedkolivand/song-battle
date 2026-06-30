import { describe, expect, it, vi } from 'vitest';
import type { Snapshot } from '@sb/types';

// battle.ts imports the IPC layer (Tauri) transitively — stub it so the module loads.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { applySnapshot } from './battle';

const snap = (seq: number): Snapshot => ({
  seq,
  battle: null,
  kick: { state: 'disconnected', channel: null },
  anonymous: false,
});

describe('applySnapshot', () => {
  it('keeps the first snapshot when there is no previous', () => {
    const s = snap(1);
    expect(applySnapshot(null, s)).toBe(s);
  });

  it('replaces the previous snapshot with a newer seq', () => {
    const prev = snap(1);
    const next = snap(2);
    expect(applySnapshot(prev, next)).toBe(next);
  });

  it('drops a stale (older) seq', () => {
    const prev = snap(5);
    expect(applySnapshot(prev, snap(3))).toBe(prev);
  });

  it('drops a duplicate (equal) seq', () => {
    const prev = snap(5);
    expect(applySnapshot(prev, snap(5))).toBe(prev);
  });
});
