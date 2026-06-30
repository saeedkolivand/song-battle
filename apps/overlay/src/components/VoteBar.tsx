import { motion } from 'framer-motion';
import { barSpring } from '../motion';

type Side = 'a' | 'b';

// Animated vote bar with percent + raw count. Slot B mirrors (fills right→left) so
// the two bars meet symmetrically around the centre. The leading side gets a subtle
// travelling shimmer (gated off under reduced motion).
export function VoteBar({
  side,
  pct,
  votes,
  leading,
  reduce,
}: {
  side: Side;
  pct: number;
  votes: number;
  leading: boolean;
  reduce: boolean;
}) {
  const fill = side === 'a' ? 'bg-gradient-to-r from-a/60 to-a' : 'bg-gradient-to-l from-b/60 to-b';
  const text = side === 'a' ? 'text-a' : 'text-b';
  const align = side === 'a' ? 'justify-start' : 'justify-end';
  const glow = side === 'a' ? 'var(--color-a)' : 'var(--color-b)';

  return (
    <div className="w-full">
      <div className={`flex items-baseline gap-[0.8vw] ${align}`}>
        <span className={`sb-shadow text-[2.6vw] font-black tabular-nums leading-none ${text}`}>{pct}%</span>
        <span className="sb-shadow text-[1.1vw] font-medium tabular-nums text-white/70">
          {votes} vote{votes === 1 ? '' : 's'}
        </span>
      </div>
      <div className={`mt-[0.6vw] flex h-[1.4vw] w-full overflow-hidden rounded-full bg-white/10 ${align}`}>
        <motion.div
          className={`relative h-full overflow-hidden rounded-full ${fill}`}
          style={leading ? { boxShadow: `0 0 1.2vw -0.2vw ${glow}` } : undefined}
          initial={false}
          animate={{ width: `${pct}%` }}
          transition={reduce ? { duration: 0 } : barSpring}
        >
          {leading && !reduce ? (
            <span
              className="pointer-events-none absolute inset-y-0 left-0 w-[28%] bg-white/35 blur-[0.25vw]"
              style={{ animation: 'sb-shimmer 2.4s ease-in-out infinite' }}
            />
          ) : null}
        </motion.div>
      </div>
    </div>
  );
}
