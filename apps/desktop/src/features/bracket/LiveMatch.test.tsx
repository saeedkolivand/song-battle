import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { MatchView, Song } from '@sb/types';
import { LiveMatch } from './LiveMatch';

const song = (id: string, title: string): Song => ({ id, title, source: 'youtube', sourceUrl: 'https://example.test' });

function match(over: Partial<MatchView> = {}): MatchView {
  return {
    id: 'm1',
    round: 1,
    a: song('a', 'Alpha'),
    b: song('b', 'Beta'),
    votesA: 0,
    votesB: 0,
    pctA: 0,
    pctB: 0,
    total: 0,
    state: 'active',
    winner: null,
    timer: null,
    group: 'main',
    bestOf: 1,
    winsA: 0,
    winsB: 0,
    ...over,
  };
}

describe('LiveMatch vote percentages', () => {
  it('renders each side percentage + vote count from the snapshot', () => {
    render(<LiveMatch match={match({ votesA: 30, votesB: 10, pctA: 75, pctB: 25, total: 40 })} />);
    expect(screen.getByLabelText('Alpha: 75 percent, 30 votes')).toBeInTheDocument();
    expect(screen.getByLabelText('Beta: 25 percent, 10 votes')).toBeInTheDocument();
  });

  it('shows 0% for both sides with no votes (0/0)', () => {
    render(<LiveMatch match={match()} />);
    expect(screen.getByLabelText('Alpha: 0 percent, 0 votes')).toBeInTheDocument();
    expect(screen.getByLabelText('Beta: 0 percent, 0 votes')).toBeInTheDocument();
  });
});
