import { beforeEach, describe, expect, it } from 'vitest';
import type { BattleView, MatchView, Snapshot } from '@sb/types';
import { useDevStore } from './devStore';

function snap(seq: number, over: Partial<Snapshot> = {}): Snapshot {
  return { seq, battle: null, kick: { state: 'disconnected', channel: null }, anonymous: false, ...over };
}

function battleWithVotes(matchId: string, votesA: number, votesB: number): BattleView {
  const m: MatchView = {
    id: matchId,
    round: 1,
    a: null,
    b: null,
    votesA,
    votesB,
    pctA: 0,
    pctB: 0,
    total: votesA + votesB,
    state: 'active',
    winner: null,
    timer: null,
    group: 'main',
    bestOf: 1,
    winsA: 0,
    winsB: 0,
  };
  return {
    id: 'b1',
    title: '',
    description: '',
    theme: '',
    mode: 'single',
    status: 'running',
    round: 1,
    totalRounds: 1,
    currentMatch: m,
    bracket: [m],
    winner: null,
    songs: [],
    songCount: 0,
  };
}

beforeEach(() => {
  useDevStore.getState().clear();
});

describe('devStore.record', () => {
  it('caps the event buffer at 300, newest first, but keeps the total count', () => {
    for (let i = 1; i <= 350; i++) useDevStore.getState().record(snap(i));
    const { events, perf } = useDevStore.getState();
    expect(events).toHaveLength(300);
    expect(events[0].seq).toBe(350); // newest first
    expect(events[events.length - 1].seq).toBe(51); // oldest retained
    expect(perf.count).toBe(350); // counts every event, past the cap
  });

  it('summarizes the first snapshot and then a delta', () => {
    useDevStore.getState().record(snap(1));
    expect(useDevStore.getState().events[0].summary).toBe('first snapshot');

    useDevStore.getState().record(snap(2, { kick: { state: 'connected', channel: 'x' } }));
    expect(useDevStore.getState().events[0].summary).toContain('kick disconnected→connected');
  });

  it('samples a vote only when A/B changes, capping at 300', () => {
    useDevStore.getState().record(snap(1, { battle: battleWithVotes('m1', 5, 5) }));
    useDevStore.getState().record(snap(2, { battle: battleWithVotes('m1', 5, 5) })); // unchanged → no sample
    expect(useDevStore.getState().votes).toHaveLength(1);

    for (let i = 3; i <= 360; i++) {
      useDevStore.getState().record(snap(i, { battle: battleWithVotes('m1', i, 0) }));
    }
    const { votes } = useDevStore.getState();
    expect(votes).toHaveLength(300);
    expect(votes[0].a).toBe(360); // newest first
  });

  it('clear() resets buffers and perf', () => {
    useDevStore.getState().record(snap(1, { battle: battleWithVotes('m1', 1, 0) }));
    useDevStore.getState().clear();
    const s = useDevStore.getState();
    expect(s.events).toHaveLength(0);
    expect(s.votes).toHaveLength(0);
    expect(s.perf.count).toBe(0);
  });
});
