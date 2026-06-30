import type { MatchView } from '@sb/types';
import { SongCard } from './components/SongCard';
import { Countdown } from './components/Countdown';

// The active-match layout: round badge, two competitor cards either side of the
// countdown ring, and the running vote total. Decided matches show a winner mark.
export function Stage({ match, totalRounds, reduce }: { match: MatchView; totalRounds: number; reduce: boolean }) {
  const decided = match.winner !== null;

  return (
    <div className="flex w-[88vw] flex-col items-center gap-[2.4vw]">
      <div className="rounded-full border border-white/15 bg-black/40 px-[1.6vw] py-[0.5vw] text-[1.1vw] font-bold uppercase tracking-[0.2em] text-white/70 backdrop-blur-md">
        Round {match.round} / {totalRounds}
      </div>

      <div className="grid w-full grid-cols-[1fr_auto_1fr] items-center gap-[3vw]">
        <SongCard
          song={match.a}
          side="a"
          pct={match.pctA}
          votes={match.votesA}
          won={match.winner === 'a'}
          reduce={reduce}
        />

        <div className="flex flex-col items-center gap-[1vw]">
          {decided ? (
            <div className="flex flex-col items-center gap-[0.4vw]">
              <span className="text-[3.5vw] leading-none">🏆</span>
              <span className="text-[1.2vw] font-bold uppercase tracking-widest text-white/70">Winner</span>
            </div>
          ) : match.timer ? (
            <Countdown timer={match.timer} reduce={reduce} />
          ) : (
            <span className="text-[3vw] font-black text-white/40">VS</span>
          )}
          <div className="text-[1vw] font-medium uppercase tracking-widest text-white/40">
            {match.total} vote{match.total === 1 ? '' : 's'}
          </div>
        </div>

        <SongCard
          song={match.b}
          side="b"
          pct={match.pctB}
          votes={match.votesB}
          won={match.winner === 'b'}
          reduce={reduce}
        />
      </div>
    </div>
  );
}
