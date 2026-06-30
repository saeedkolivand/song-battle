import { create } from 'zustand';
import type { Snapshot } from '@sb/types';

// RAM-only observability over the snapshot stream. Every buffer is hard-capped so
// nothing grows unbounded — this is the perf-conscious path (no virtualization lib).
const EVENT_CAP = 300;
const VOTE_CAP = 300;
const RATE_WINDOW_MS = 5000;

export interface EventEntry {
  seq: number;
  ts: number; // wall-clock ms (display)
  summary: string;
}

export interface VoteEntry {
  ts: number;
  matchId: string;
  a: number;
  b: number;
}

export interface Perf {
  rate: number; // snapshots/sec, rolling over ~5s
  lastGapMs: number; // inter-event gap
  heapMB: number | null; // null = unavailable (non-Chromium)
  count: number; // total events since start
}

interface DevState {
  events: EventEntry[]; // newest first
  votes: VoteEntry[]; // newest first
  perf: Perf;
  record: (snap: Snapshot) => void;
  clear: () => void;
}

// `performance.memory` is Chromium/WebView2-only and absent from TS lib types — read
// it through a narrow typed accessor (no `any`), degrading to null elsewhere.
interface MemoryInfo {
  usedJSHeapSize: number;
}
function readHeapMB(): number | null {
  const mem = (performance as Performance & { memory?: MemoryInfo }).memory;
  return mem ? Math.round(mem.usedJSHeapSize / (1024 * 1024)) : null;
}

function pushCapped<T>(buffer: T[], entry: T, cap: number): T[] {
  return [entry, ...buffer].slice(0, cap);
}

function describeDelta(prev: Snapshot | null, cur: Snapshot): string {
  if (!prev) return 'first snapshot';
  const parts: string[] = [];
  const pb = prev.battle;
  const cb = cur.battle;
  if (!pb && cb) parts.push('battle created');
  if (pb && !cb) parts.push('battle cleared');
  if (pb && cb) {
    if (pb.status !== cb.status) parts.push(`status ${pb.status}→${cb.status}`);
    if (pb.round !== cb.round) parts.push(`round ${pb.round}→${cb.round}`);
    const pm = pb.currentMatch;
    const cm = cb.currentMatch;
    if ((pm?.id ?? null) !== (cm?.id ?? null)) {
      parts.push(`match→${cm ? cm.id.slice(0, 8) : 'none'}`);
    } else if (pm && cm && pm.state !== cm.state) {
      parts.push(`match ${pm.state}→${cm.state}`);
    }
    if (cm && (cm.votesA !== (pm?.votesA ?? -1) || cm.votesB !== (pm?.votesB ?? -1))) {
      parts.push(`votes ${cm.votesA}/${cm.votesB}`);
    }
  }
  if (prev.kick.state !== cur.kick.state) parts.push(`kick ${prev.kick.state}→${cur.kick.state}`);
  if (prev.anonymous !== cur.anonymous) parts.push(`anonymous ${String(cur.anonymous)}`);
  return parts.length > 0 ? parts.join(' · ') : 'no change';
}

export const useDevStore = create<DevState>((set, get) => {
  // Bookkeeping kept outside store state so it never triggers re-renders.
  let prevSnap: Snapshot | null = null;
  let lastPerfTs: number | null = null;
  let rateWindow: number[] = [];
  let totalCount = 0;
  let lastVote = { matchId: '', a: -1, b: -1 };

  return {
    events: [],
    votes: [],
    perf: { rate: 0, lastGapMs: 0, heapMB: readHeapMB(), count: 0 },

    record: (snap) => {
      const nowPerf = performance.now();
      const nowWall = Date.now();

      const gap = lastPerfTs === null ? 0 : nowPerf - lastPerfTs;
      lastPerfTs = nowPerf;
      rateWindow.push(nowPerf);
      while (rateWindow.length > 0 && nowPerf - rateWindow[0] > RATE_WINDOW_MS) rateWindow.shift();
      const rate = rateWindow.length / (RATE_WINDOW_MS / 1000);
      totalCount += 1;

      const summary = describeDelta(prevSnap, snap);
      const events = pushCapped(get().events, { seq: snap.seq, ts: nowWall, summary }, EVENT_CAP);

      let votes = get().votes;
      const cur = snap.battle?.currentMatch ?? null;
      if (cur && (cur.id !== lastVote.matchId || cur.votesA !== lastVote.a || cur.votesB !== lastVote.b)) {
        votes = pushCapped(votes, { ts: nowWall, matchId: cur.id, a: cur.votesA, b: cur.votesB }, VOTE_CAP);
        lastVote = { matchId: cur.id, a: cur.votesA, b: cur.votesB };
      }

      prevSnap = snap;
      set({ events, votes, perf: { rate, lastGapMs: gap, heapMB: readHeapMB(), count: totalCount } });
    },

    clear: () => {
      prevSnap = null;
      lastPerfTs = null;
      rateWindow = [];
      totalCount = 0;
      lastVote = { matchId: '', a: -1, b: -1 };
      set({ events: [], votes: [], perf: { rate: 0, lastGapMs: 0, heapMB: readHeapMB(), count: 0 } });
    },
  };
});
