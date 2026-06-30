import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { EmptyState, SeriesScore } from './common';

describe('SeriesScore', () => {
  it('renders nothing for a single game (bestOf 1)', () => {
    const { container } = render(<SeriesScore winsA={0} winsB={0} bestOf={1} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the score, an accessible label, and the right pip count for a 2-1 series', () => {
    render(<SeriesScore winsA={2} winsB={1} bestOf={3} />);
    const el = screen.getByLabelText('Series score 2 to 1, best of 3');
    expect(el).toHaveTextContent('2–1');
    // best-of-3 needs 2 wins → 2 pips per side → 4 decorative pips total
    expect(el.querySelectorAll('span[aria-hidden="true"] > span')).toHaveLength(4);
  });
});

describe('EmptyState', () => {
  it('renders the title and optional hint', () => {
    render(<EmptyState title="Nothing here" hint="Add something" />);
    expect(screen.getByText('Nothing here')).toBeInTheDocument();
    expect(screen.getByText('Add something')).toBeInTheDocument();
  });
});
