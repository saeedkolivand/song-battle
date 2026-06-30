import { useEffect, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import type { MotionProps } from 'framer-motion';
import { connectOverlay } from '@sb/shared';
import type { Snapshot, Song } from '@sb/types';
import { Stage } from './Stage';
import { matchSwap, swapTransition } from './motion';

// The overlay is a dumb projection of server snapshots over the axum WS. It
// derives the WS URL from its own origin so it works on any port OBS loads.
export function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [open, setOpen] = useState(false);
  const reduce = useReducedMotion() ?? false;

  useEffect(() => connectOverlay(`ws://${location.host}/ws`, setSnap, setOpen), []);

  const battle = snap?.battle ?? null;
  const match = battle?.currentMatch ?? null;
  const finished = battle?.status === 'finished' && battle.winner;

  // Reduced motion: opacity-only crossfade, no positional jump.
  const anim: MotionProps = reduce
    ? { initial: { opacity: 0 }, animate: { opacity: 1 }, exit: { opacity: 0 }, transition: { duration: 0.15 } }
    : { variants: matchSwap, initial: 'initial', animate: 'animate', exit: 'exit', transition: swapTransition };

  return (
    <div className="flex h-full w-full items-center justify-center overflow-hidden p-[3vw] text-white">
      <AnimatePresence mode="wait">
        {finished && battle?.winner ? (
          <motion.div key="winner" {...anim}>
            <WinnerCard song={battle.winner} />
          </motion.div>
        ) : match ? (
          <motion.div key={match.id} className="w-full" {...anim}>
            <Stage match={match} totalRounds={battle?.totalRounds ?? 1} reduce={reduce} />
          </motion.div>
        ) : (
          <motion.div key="idle" {...anim}>
            <IdleBadge connected={open} title={battle?.title ?? null} />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function WinnerCard({ song }: { song: Song }) {
  return (
    <div className="flex flex-col items-center gap-[1.5vw] text-center">
      <span className="text-[1.4vw] font-bold uppercase tracking-[0.3em] text-white/60">Champion</span>
      <div className="overflow-hidden rounded-[1.5vw] border border-white/15 bg-black/40 shadow-2xl ring-4 ring-a backdrop-blur-md">
        <div className="h-[26vw] w-[26vw] bg-white/5">
          {song.thumbnail ? (
            <img src={song.thumbnail} alt="" className="h-full w-full object-cover" />
          ) : (
            <div className="flex h-full w-full items-center justify-center text-[6vw] text-white/20">♪</div>
          )}
        </div>
      </div>
      <div>
        <div className="text-[3vw] font-black leading-tight">{song.title}</div>
        <div className="text-[1.6vw] text-white/55">{song.artist ?? ''}</div>
      </div>
    </div>
  );
}

function IdleBadge({ connected, title }: { connected: boolean; title: string | null }) {
  return (
    <div className="flex flex-col items-center gap-[0.8vw] rounded-[1.5vw] border border-white/10 bg-black/30 px-[3vw] py-[2vw] text-center backdrop-blur-md">
      <span className="text-[2vw] font-black tracking-tight text-white/80">{title ?? 'Song Battle'}</span>
      <span className="flex items-center gap-[0.6vw] text-[1vw] text-white/40">
        <span className={`h-[0.7vw] w-[0.7vw] rounded-full ${connected ? 'bg-a' : 'bg-amber-400'}`} aria-hidden="true" />
        {connected ? 'Waiting for the next match…' : 'Connecting…'}
      </span>
    </div>
  );
}
