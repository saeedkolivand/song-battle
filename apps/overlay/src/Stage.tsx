import { motion } from 'framer-motion';
import type { MatchView } from '@sb/types';
import { groupLabel } from '@sb/shared';
import { SongCard } from './components/SongCard';
import { Countdown } from './components/Countdown';
import { SeriesScore } from './components/SeriesScore';
import { cardItem, cardItemReduced, cardsContainer } from './motion';

// The active-match layout: round badge, two competitor cards either side of the
// countdown ring, and the running vote total. Decided matches show a winner mark.
export function Stage({ match, totalRounds, reduce }: { match: MatchView; totalRounds: number; reduce: boolean }) {
  const decided = match.winner !== null;
  const group = groupLabel(match.group);
  const item = reduce ? cardItemReduced : cardItem;

  const leadA = !decided && match.votesA > match.votesB;
  const leadB = !decided && match.votesB > match.votesA;

  return (
    <div className="flex w-[88vw] flex-col items-center gap-[2.4vw]">
      <div className="flex items-center gap-[1vw]">
        <div className="sb-shadow rounded-full border border-white/15 bg-black/40 px-[1.6vw] py-[0.5vw] text-[1.1vw] font-bold uppercase tracking-[0.2em] text-white/80 backdrop-blur-md">
          Round {match.round} / {totalRounds}
        </div>
        {group ? (
          <div className="sb-shadow rounded-full border border-accent/30 bg-black/40 px-[1.4vw] py-[0.5vw] text-[1vw] font-bold uppercase tracking-[0.2em] text-accent backdrop-blur-md">
            {group}
          </div>
        ) : null}
      </div>

      <motion.div
        className="grid w-full grid-cols-[1fr_auto_1fr] items-center gap-[3vw]"
        variants={cardsContainer}
        initial="initial"
        animate="animate"
      >
        <motion.div variants={item}>
          <SongCard
            song={match.a}
            side="a"
            pct={match.pctA}
            votes={match.votesA}
            won={match.winner === 'a'}
            lost={decided && match.winner !== 'a'}
            leading={leadA}
            reduce={reduce}
          />
        </motion.div>

        <motion.div variants={item} className="flex flex-col items-center gap-[1vw]">
          {decided ? (
            <div className="flex flex-col items-center gap-[0.4vw]">
              <span className="text-[3.5vw] leading-none">🏆</span>
              <span className="sb-shadow text-[1.2vw] font-bold uppercase tracking-widest text-accent">Winner</span>
            </div>
          ) : match.timer ? (
            <Countdown timer={match.timer} reduce={reduce} />
          ) : (
            <span className="sb-shadow text-[3vw] font-black text-white/50">VS</span>
          )}
          <div className="sb-shadow text-[1vw] font-medium uppercase tracking-widest text-white/60">
            {match.total} vote{match.total === 1 ? '' : 's'}
          </div>
          {match.bestOf > 1 ? (
            <SeriesScore winsA={match.winsA} winsB={match.winsB} bestOf={match.bestOf} />
          ) : null}
        </motion.div>

        <motion.div variants={item}>
          <SongCard
            song={match.b}
            side="b"
            pct={match.pctB}
            votes={match.votesB}
            won={match.winner === 'b'}
            lost={decided && match.winner !== 'b'}
            leading={leadB}
            reduce={reduce}
          />
        </motion.div>
      </motion.div>
    </div>
  );
}
