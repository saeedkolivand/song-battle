import { useEffect, useState } from 'react';
import { connectOverlay } from '@sb/shared';
import { Card } from '@sb/ui';
import type { Snapshot } from '@sb/types';

// Phase 0 proof-of-pipe: the overlay is a dumb projection of server snapshots.
// It derives the WS URL from its own origin (axum serves both the page and /ws),
// so it works unchanged whether OBS loads localhost:31337 or a future port.
export function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => connectOverlay(`ws://${location.host}/ws`, setSnap, setOpen), []);

  return (
    <div className="flex h-full items-center justify-center text-white">
      <Card className="text-center">
        <div className="text-xs uppercase tracking-widest opacity-60">
          Song Battle · {open ? 'connected' : 'reconnecting…'}
        </div>
        <div className="mt-2 text-7xl font-black tabular-nums">{snap?.counter ?? '—'}</div>
        <div className="mt-1 text-xs opacity-40">seq {snap?.seq ?? 0}</div>
      </Card>
    </div>
  );
}
