import type { MatchView, Song } from '@sb/types';
import { mmss } from '@sb/shared';
import { MatchPill, Pill } from '../../components/common';

function slotName(song: Song | null, fallback: string): string {
  return song?.title ?? fallback;
}

function VoteRow({
  label,
  pct,
  votes,
  color,
  won,
}: {
  label: string;
  pct: number;
  votes: number;
  color: string;
  won: boolean;
}) {
  return (
    <div className={won ? 'rounded-xl ring-1 ring-accent/50' : ''}>
      <div className="flex items-baseline justify-between gap-3 px-1">
        <span className="truncate text-sm font-medium text-white">{label}</span>
        <span className="shrink-0 text-sm tabular-nums text-white/60">
          {pct}% · {votes}
        </span>
      </div>
      <div className="mt-1.5 h-3 overflow-hidden rounded-full bg-white/10">
        <div
          className={`h-full rounded-full ${color} transition-[width] duration-500 ease-out`}
          style={{ width: `${pct}%` }}
          role="progressbar"
          aria-valuenow={pct}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={`${label}: ${pct} percent, ${votes} votes`}
        />
      </div>
    </div>
  );
}

export function LiveMatch({ match, anonymous = false }: { match: MatchView; anonymous?: boolean }) {
  const timer = match.timer;
  const urgent = timer ? timer.running && timer.remainingSec <= 5 : false;

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-center justify-between">
        <span className="text-xs uppercase tracking-wider text-white/40">Round {match.round}</span>
        <div className="flex items-center gap-2">
          {anonymous ? <Pill tone="idle">anonymous</Pill> : null}
          <MatchPill state={match.state} />
        </div>
      </div>

      <div className="flex flex-col gap-4">
        <VoteRow
          label={slotName(match.a, 'Slot A')}
          pct={match.pctA}
          votes={match.votesA}
          color="bg-accent"
          won={match.winner === 'a'}
        />
        <VoteRow
          label={slotName(match.b, 'Slot B')}
          pct={match.pctB}
          votes={match.votesB}
          color="bg-sky-400"
          won={match.winner === 'b'}
        />
      </div>

      <div className="flex items-center justify-between border-t border-white/10 pt-4 text-sm">
        <span className="text-white/50">
          {match.total} vote{match.total === 1 ? '' : 's'}
        </span>
        {timer ? (
          <span
            className={`text-2xl font-black tabular-nums ${
              urgent ? 'animate-pulse text-red-400 motion-reduce:animate-none' : 'text-white'
            }`}
          >
            {mmss(timer.remainingSec)}
          </span>
        ) : (
          <span className="text-white/40">no timer</span>
        )}
      </div>
    </div>
  );
}
