import { motion } from 'framer-motion';
import type { TimerView } from '@sb/types';
import { mmss } from '@sb/shared';

const R = 45;
const CIRC = 2 * Math.PI * R;

// Countdown ring + number. Pulses in the final 5 seconds. The ring drains as time
// runs out; colour shifts to red when urgent.
export function Countdown({ timer, reduce }: { timer: TimerView; reduce: boolean }) {
  const progress = timer.durationSec > 0 ? Math.max(0, Math.min(1, timer.remainingSec / timer.durationSec)) : 0;
  const offset = CIRC * (1 - progress);
  const urgent = timer.running && timer.remainingSec <= 5;

  return (
    <motion.div
      className="relative h-[11vw] w-[11vw]"
      role="img"
      aria-label={`${Math.ceil(timer.remainingSec)} seconds remaining`}
      animate={urgent && !reduce ? { scale: [1, 1.1, 1] } : { scale: 1 }}
      transition={urgent && !reduce ? { duration: 1, repeat: Infinity, ease: 'easeInOut' } : { duration: 0 }}
    >
      <svg viewBox="0 0 100 100" className="h-full w-full -rotate-90">
        <circle cx="50" cy="50" r={R} fill="none" stroke="rgba(255,255,255,0.12)" strokeWidth="7" />
        <motion.circle
          cx="50"
          cy="50"
          r={R}
          fill="none"
          stroke={urgent ? '#f87171' : 'currentColor'}
          className={urgent ? '' : 'text-accent'}
          strokeWidth="7"
          strokeLinecap="round"
          strokeDasharray={CIRC}
          initial={false}
          animate={{ strokeDashoffset: offset }}
          transition={reduce ? { duration: 0 } : { duration: 0.4, ease: 'linear' }}
        />
      </svg>
      <div
        className={`sb-shadow absolute inset-0 flex items-center justify-center text-[3vw] font-black tabular-nums ${
          urgent ? 'text-red-400' : 'text-white'
        }`}
      >
        {mmss(timer.remainingSec)}
      </div>
    </motion.div>
  );
}
