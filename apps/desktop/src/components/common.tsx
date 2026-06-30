import type { ReactNode } from 'react';
import type { BattleStatus, ConnectionState, MatchState } from '@sb/types';

export function PageHeader({ title, subtitle }: { title: string; subtitle?: string }) {
  return (
    <header className="mb-6">
      <h1 className="text-2xl font-black tracking-tight text-white">{title}</h1>
      {subtitle ? <p className="mt-1 text-sm text-white/50">{subtitle}</p> : null}
    </header>
  );
}

export function Section({
  title,
  action,
  children,
  className = '',
}: {
  title?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-2xl border border-white/10 bg-white/5 p-6 shadow-xl backdrop-blur-md ${className}`}>
      {title || action ? (
        <div className="mb-4 flex items-center justify-between gap-3">
          {title ? <h2 className="text-sm font-semibold uppercase tracking-wider text-white/60">{title}</h2> : <span />}
          {action}
        </div>
      ) : null}
      {children}
    </section>
  );
}

export function Stat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="rounded-xl border border-white/10 bg-black/20 px-4 py-3">
      <div className="text-xs uppercase tracking-wider text-white/40">{label}</div>
      <div className="mt-1 text-lg font-semibold text-white">{value}</div>
    </div>
  );
}

type Tone = 'idle' | 'good' | 'warn' | 'bad' | 'live';

const toneClass: Record<Tone, string> = {
  idle: 'border-white/15 bg-white/5 text-white/60',
  good: 'border-emerald-400/30 bg-emerald-400/10 text-emerald-300',
  warn: 'border-amber-400/30 bg-amber-400/10 text-amber-300',
  bad: 'border-red-400/30 bg-red-400/10 text-red-300',
  live: 'border-accent/40 bg-accent/10 text-accent',
};

export function Pill({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${toneClass[tone]}`}>
      {children}
    </span>
  );
}

const connectionTone: Record<ConnectionState, Tone> = {
  disconnected: 'idle',
  connecting: 'warn',
  connected: 'good',
  reconnecting: 'warn',
  error: 'bad',
};

export function ConnectionPill({ state }: { state: ConnectionState }) {
  return <Pill tone={connectionTone[state]}>{state}</Pill>;
}

const battleTone: Record<BattleStatus, Tone> = { idle: 'idle', running: 'live', finished: 'good' };
export function BattlePill({ status }: { status: BattleStatus }) {
  return <Pill tone={battleTone[status]}>{status}</Pill>;
}

const matchTone: Record<MatchState, Tone> = { pending: 'idle', active: 'live', done: 'good' };
export function MatchPill({ state }: { state: MatchState }) {
  return <Pill tone={matchTone[state]}>{state}</Pill>;
}

function Pips({ filled, total, fillClass }: { filled: number; total: number; fillClass: string }) {
  return (
    <span className="inline-flex gap-0.5" aria-hidden="true">
      {Array.from({ length: total }, (_, i) => (
        <span key={i} className={`h-2 w-2 rounded-full ${i < filled ? fillClass : 'bg-white/20'}`} />
      ))}
    </span>
  );
}

// Series score for best-of matches: win pips per side + the wins tally. Returns
// null for single games (bestOf 1) so callers can render it unconditionally.
export function SeriesScore({ winsA, winsB, bestOf }: { winsA: number; winsB: number; bestOf: number }) {
  if (bestOf <= 1) return null;
  const need = Math.floor(bestOf / 2) + 1;
  return (
    <span className="inline-flex items-center gap-2" aria-label={`Series score ${winsA} to ${winsB}, best of ${bestOf}`}>
      <Pips filled={winsA} total={need} fillClass="bg-accent" />
      <span className="text-xs font-semibold tabular-nums text-white/80">
        {winsA}–{winsB}
      </span>
      <Pips filled={winsB} total={need} fillClass="bg-sky-400" />
    </span>
  );
}

export function ErrorNote({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <p role="alert" className="rounded-lg border border-red-400/30 bg-red-400/10 px-3 py-2 text-sm text-red-300">
      {message}
    </p>
  );
}

export function EmptyState({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-white/15 bg-white/[0.02] px-6 py-12 text-center">
      <p className="text-sm font-medium text-white/70">{title}</p>
      {hint ? <p className="mx-auto mt-1 max-w-md text-sm text-white/40">{hint}</p> : null}
    </div>
  );
}
