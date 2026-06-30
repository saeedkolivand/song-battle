import { describe, expect, it } from 'vitest';
import { mmss, clampPct, modeLabel, groupLabel } from '@sb/shared';

describe('mmss', () => {
  it('formats seconds as m:ss', () => {
    expect(mmss(0)).toBe('0:00');
    expect(mmss(5)).toBe('0:05');
    expect(mmss(75)).toBe('1:15');
    expect(mmss(600)).toBe('10:00');
  });

  it('clamps negatives to 0:00 and floors fractions', () => {
    expect(mmss(-10)).toBe('0:00');
    expect(mmss(9.9)).toBe('0:09');
  });
});

describe('clampPct', () => {
  it('clamps into 0..100 and rounds', () => {
    expect(clampPct(-5)).toBe(0);
    expect(clampPct(150)).toBe(100);
    expect(clampPct(42.4)).toBe(42);
    expect(clampPct(42.6)).toBe(43);
  });
});

describe('modeLabel', () => {
  it('maps each tournament mode', () => {
    expect(modeLabel('single')).toBe('Single Elimination');
    expect(modeLabel('double')).toBe('Double Elimination');
    expect(modeLabel('bo3')).toBe('Best of 3');
  });
});

describe('groupLabel', () => {
  it('labels the double-elim sub-brackets and returns null for main', () => {
    expect(groupLabel('winners')).toBe('Winners');
    expect(groupLabel('losers')).toBe('Losers');
    expect(groupLabel('grand')).toBe('Grand Final');
    expect(groupLabel('main')).toBeNull();
  });
});
