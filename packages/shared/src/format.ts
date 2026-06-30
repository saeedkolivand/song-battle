// Tiny presentation helpers shared by both frontends. Pure, no deps.

/** Seconds → `m:ss` (clamped at 0). e.g. 75 → "1:15", 5 → "0:05". */
export function mmss(totalSec: number): string {
  const s = Math.max(0, Math.floor(totalSec));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, '0')}`;
}

/** Clamp an integer percentage into 0..100. */
export function clampPct(n: number): number {
  return Math.max(0, Math.min(100, Math.round(n)));
}
