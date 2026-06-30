import type { Song } from '@sb/types';
import { VoteBar } from './VoteBar';

type Side = 'a' | 'b';

// One competitor: artwork, title, artist, and its vote bar. Highlights with a
// coloured ring when it has won the match.
export function SongCard({
  song,
  side,
  pct,
  votes,
  won,
  reduce,
}: {
  song: Song | null;
  side: Side;
  pct: number;
  votes: number;
  won: boolean;
  reduce: boolean;
}) {
  const ring = side === 'a' ? 'ring-a' : 'ring-b';

  return (
    <div className="flex w-full flex-col gap-[1vw]">
      <div
        className={`overflow-hidden rounded-[1.2vw] border border-white/10 bg-black/40 shadow-2xl backdrop-blur-md transition-shadow ${
          won ? `ring-4 ${ring}` : ''
        }`}
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
      <VoteBar side={side} pct={pct} votes={votes} reduce={reduce} />
    </div>
  );
}
