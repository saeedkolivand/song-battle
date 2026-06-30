import { useEffect, useRef } from 'react';
import confetti from 'canvas-confetti';
import { COLOR_A, COLOR_ACCENT, COLOR_B } from '../colors';

export interface Celebration {
  key: string; // identity of the win — fire once per distinct key
  champion: boolean; // bigger burst for the overall champion
}

const COLORS = [COLOR_ACCENT, COLOR_A, COLOR_B];

// Returns a cancel fn that stops the champion volley's rAF loop (no-op otherwise),
// so it can't keep firing on a reset canvas after unmount / next burst.
function burst(fire: confetti.CreateTypes, champion: boolean): () => void {
  const base = { colors: COLORS, disableForReducedMotion: true, ticks: 220 } as const;
  if (!champion) {
    void fire({ ...base, particleCount: 90, spread: 80, startVelocity: 42, origin: { x: 0.5, y: 0.5 } });
    void fire({ ...base, particleCount: 40, angle: 60, spread: 55, origin: { x: 0.15, y: 0.62 } });
    void fire({ ...base, particleCount: 40, angle: 120, spread: 55, origin: { x: 0.85, y: 0.62 } });
    return () => {};
  }
  // Champion: a big centre pop plus ~1.2s of side cannons.
  void fire({ ...base, particleCount: 180, spread: 110, startVelocity: 48, scalar: 1.15, origin: { x: 0.5, y: 0.42 } });
  const end = Date.now() + 1200;
  let raf = 0;
  const volley = () => {
    void fire({ ...base, particleCount: 7, angle: 60, spread: 65, startVelocity: 55, origin: { x: 0, y: 0.7 } });
    void fire({ ...base, particleCount: 7, angle: 120, spread: 65, startVelocity: 55, origin: { x: 1, y: 0.7 } });
    if (Date.now() < end) raf = requestAnimationFrame(volley);
  };
  volley();
  return () => cancelAnimationFrame(raf);
}

/**
 * Full-screen, transparent, pointer-events-none confetti canvas. Fires once per
 * distinct `celebration.key` (de-duped) and is skipped entirely under reduced motion.
 * Driven by the canvas-confetti rAF loop — no React state churn.
 */
export function WinnerConfetti({ celebration, reduce }: { celebration: Celebration | null; reduce: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const fireRef = useRef<confetti.CreateTypes | null>(null);
  const lastKey = useRef<string | null>(null);
  const cancelRef = useRef<() => void>(() => {});

  useEffect(() => {
    if (!canvasRef.current) return;
    const instance = confetti.create(canvasRef.current, { resize: true, useWorker: true });
    fireRef.current = instance;
    return () => {
      cancelRef.current(); // stop any in-flight champion volley before reset
      instance.reset();
      fireRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (reduce || !celebration || celebration.key === lastKey.current) return;
    lastKey.current = celebration.key;
    const fire = fireRef.current;
    if (fire) {
      cancelRef.current(); // cancel a prior volley before starting a new burst
      cancelRef.current = burst(fire, celebration.champion);
    }
  }, [celebration, reduce]);

  return <canvas ref={canvasRef} aria-hidden="true" className="pointer-events-none fixed inset-0 z-50 h-full w-full" />;
}
