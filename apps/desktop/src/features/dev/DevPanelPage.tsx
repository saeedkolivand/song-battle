import { useEffect, useMemo, useState } from 'react';
import type { ConnectionState } from '@sb/types';
import { Button } from '@sb/ui';
import { useBattleStore } from '../../stores/battle';
import { useObsStore } from '../obs/useObs';
import { PageHeader, Section, Stat, ConnectionPill, EmptyState } from '../../components/common';
import { useDevStore } from './devStore';

function clock(ts: number): string {
  return new Date(ts).toLocaleTimeString();
}

export function DevPanelPage() {
  const snapshot = useBattleStore((s) => s.snapshot);
  const obsState = useObsStore((s) => s.state);
  const events = useDevStore((s) => s.events);
  const votes = useDevStore((s) => s.votes);
  const perf = useDevStore((s) => s.perf);
  const clear = useDevStore((s) => s.clear);
  // Recomputed only when the snapshot actually changes, not on every 1s tick re-render.
  const snapshotJson = useMemo(() => (snapshot ? JSON.stringify(snapshot, null, 2) : ''), [snapshot]);

  // Tick once a second so "last event Ns ago" stays current while the page is open.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const lastEventTs = events.length > 0 ? events[0].ts : null;
  const streamState: ConnectionState =
    lastEventTs === null ? 'disconnected' : now - lastEventTs < 5000 ? 'connected' : 'reconnecting';
  const lastAgo = lastEventTs === null ? '—' : `${Math.round((now - lastEventTs) / 1000)}s ago`;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Dev" subtitle="Live observability over the snapshot/event stream (in-memory, this session only)." />

      <Section title="Connections">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <div className="flex items-center justify-between rounded-xl border border-white/10 bg-black/20 px-4 py-3">
            <span className="text-sm text-white/60">Kick</span>
            <ConnectionPill state={snapshot?.kick.state ?? 'disconnected'} />
          </div>
          <div className="flex items-center justify-between rounded-xl border border-white/10 bg-black/20 px-4 py-3">
            <span className="text-sm text-white/60">OBS</span>
            <ConnectionPill state={obsState} />
          </div>
          <div className="flex items-center justify-between rounded-xl border border-white/10 bg-black/20 px-4 py-3">
            <span className="text-sm text-white/60">Snapshot stream</span>
            <span className="flex items-center gap-2">
              <span className="text-xs text-white/40">{lastAgo}</span>
              <ConnectionPill state={streamState} />
            </span>
          </div>
        </div>
      </Section>

      <Section title="Performance">
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <Stat label="Snapshots/sec" value={perf.rate.toFixed(1)} />
          <Stat label="Last gap" value={`${Math.round(perf.lastGapMs)} ms`} />
          <Stat label="JS heap" value={perf.heapMB === null ? 'n/a' : `${perf.heapMB} MB`} />
          <Stat label="Total events" value={perf.count} />
        </div>
      </Section>

      <Section
        title="Event log"
        action={
          <Button size="sm" variant="ghost" onClick={clear} disabled={events.length === 0 && votes.length === 0}>
            Clear
          </Button>
        }
      >
        {events.length === 0 ? (
          <EmptyState title="No events yet" hint="Snapshots will appear here as the backend emits them." />
        ) : (
          <ul className="max-h-80 overflow-y-auto rounded-xl border border-white/10 bg-black/30 p-3 font-mono text-xs">
            {events.map((e) => (
              <li key={e.seq} className="flex gap-3 border-b border-white/5 py-1 last:border-0">
                <span className="shrink-0 tabular-nums text-white/40">#{e.seq}</span>
                <span className="shrink-0 tabular-nums text-white/40">{clock(e.ts)}</span>
                <span className="text-white/80">{e.summary}</span>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="Vote log">
        {votes.length === 0 ? (
          <EmptyState title="No vote samples" hint="Recorded when the current match's A/B counts change." />
        ) : (
          <ul className="max-h-64 overflow-y-auto rounded-xl border border-white/10 bg-black/30 p-3 font-mono text-xs">
            {votes.map((v, i) => (
              <li key={`${v.matchId}-${i}`} className="flex gap-3 border-b border-white/5 py-1 last:border-0">
                <span className="shrink-0 tabular-nums text-white/40">{clock(v.ts)}</span>
                <span className="shrink-0 text-accent">A {v.a}</span>
                <span className="shrink-0 text-sky-400">B {v.b}</span>
                <span className="truncate text-white/40">{v.matchId.slice(0, 8)}</span>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="Current state">
        {snapshot ? (
          <details className="group">
            <summary className="cursor-pointer select-none rounded-lg px-2 py-1 text-sm text-white/70 hover:bg-white/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent">
              <span className="ml-1">Latest snapshot JSON (seq {snapshot.seq})</span>
            </summary>
            <pre className="mt-3 max-h-96 overflow-auto rounded-xl border border-white/10 bg-black/40 p-3 font-mono text-xs text-white/70">
              {snapshotJson}
            </pre>
          </details>
        ) : (
          <EmptyState title="No snapshot yet" />
        )}
      </Section>
    </div>
  );
}
