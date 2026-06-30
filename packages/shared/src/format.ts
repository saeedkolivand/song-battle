// Tiny presentation helpers shared by both frontends. Pure functions only.
import type { BattleMode, MatchGroup } from '@sb/types';

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

/** Epoch-ms timestamp → locale date+time, e.g. "30 Jun 2026, 14:05". */
export function formatDateTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

/** Human label for a battle mode. */
export function modeLabel(mode: BattleMode): string {
  switch (mode) {
    case 'single':
      return 'Single Elimination';
    case 'double':
      return 'Double Elimination';
    case 'bo3':
      return 'Best of 3';
  }
}

/** Human label for a double-elim sub-bracket, or null for the single-elim 'main' group. */
export function groupLabel(group: MatchGroup): string | null {
  switch (group) {
    case 'winners':
      return 'Winners';
    case 'losers':
      return 'Losers';
    case 'grand':
      return 'Grand Final';
    case 'main':
      return null;
  }
}
