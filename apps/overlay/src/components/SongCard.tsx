import type { Song } from '@sb/types';
import { VoteBar } from './VoteBar';
import { COLOR_A, COLOR_B } from '../colors';

type Side = 'a' | 'b';

// One competitor: artwork, title, artist, and its vote bar. The winner gets a
// coloured ring + glow + slight scale-up; the loser dims and desaturates.
export function SongCard({
  song,
  side,
  pct,
  votes,
  won,
  lost,
  leading,
  reduce,
}: {
  song: Song | null;
  side: Side;
  pct: number;
  votes: number;
  won: boolean;
  lost: boolean;
  leading: boolean;
  reduce: boolean;
}) {
  const ring = side === 'a' ? 'ring-a' : 'ring-b';
  const color = side === 'a' ? COLOR_A : COLOR_B;

  const stateClass = won ? `scale-[1.04] ring-4 ${ring}` : lost ? 'scale-[0.97] opacity-50 saturate-50' : '';

  return (
    <div className="flex w-full flex-col gap-[1vw]">
      <div
        className={`overflow-hidden rounded-[1.2vw] border border-white/10 bg-black/40 shadow-2xl backdrop-blur-md transition-all duration-500 ${stateClass}`}
        style={won ? { boxShadow: `0 0 4vw -0.8vw ${color}` } : undefined}
      >
        <div className="aspect-square w-full bg-white/5">
          {song?.thumbnail ? (
            <img src={song.thumbnail} alt="" className="h-full w-full object-cover" />
          ) : (
            <div className="flex h-full w-full items-center justify-center text-[3vw] text-white/20">♪</div>
          )}
        </div>
        <div className="px-[1.2vw] py-[1vw]">
          <div className="truncate text-[1.9vw] font-black leading-tight text-white">{song?.title ?? 'TBD'}</div>
          <div className="truncate text-[1.1vw] text-white/55">{song?.artist ?? '—'}</div>
        </div>
      </div>
      <VoteBar side={side} pct={pct} votes={votes} leading={leading} reduce={reduce} />
    </div>
  );
}
