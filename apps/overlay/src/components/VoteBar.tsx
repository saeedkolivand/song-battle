import { motion } from 'framer-motion';
import { barSpring } from '../motion';

type Side = 'a' | 'b';

// Animated vote bar with percent + raw count. Slot B mirrors (fills right→left)
// so the two bars meet symmetrically around the centre.
export function VoteBar({
  side,
  pct,
  votes,
  reduce,
}: {
  side: Side;
  pct: number;
  votes: number;
  reduce: boolean;
}) {
  const fill = side === 'a' ? 'bg-a' : 'bg-b';
  const text = side === 'a' ? 'text-a' : 'text-b';
  const align = side === 'a' ? 'justify-start' : 'justify-end';

  return (
    <div className="w-full">
      <div className={`flex items-baseline gap-[0.8vw] ${align}`}>
        <span className={`text-[2.6vw] font-black tabular-nums leading-none ${text}`}>{pct}%</span>
        <span className="text-[1.1vw] font-medium tabular-nums text-white/60">
          {votes} vote{votes === 1 ? '' : 's'}
        </span>
      </div>
      <div className={`mt-[0.6vw] flex h-[1.4vw] w-full overflow-hidden rounded-full bg-white/10 ${align}`}>
        <motion.div
          className={`h-full rounded-full ${fill}`}
          initial={false}
          animate={{ width: `${pct}%` }}
          transition={reduce ? { duration: 0 } : barSpring}
        />
      </div>
    </div>
  );
}
