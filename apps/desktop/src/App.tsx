import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Card } from '@sb/ui';

// Phase 0 dashboard: proves IPC (ping) and surfaces the overlay URL for OBS.
// Real pages (Home/Battle/Songs/Bracket/Overlay/OBS/Kick/Settings/Logs/About)
// arrive in Phase 1.
export default function App() {
  const [pong, setPong] = useState('…');
  const [overlay, setOverlay] = useState('');

  useEffect(() => {
    invoke<string>('ping')
      .then(setPong)
      .catch((e) => setPong(`error: ${e}`));
    invoke<string>('overlay_url').then(setOverlay).catch(() => {});
  }, []);

  return (
    <main className="mx-auto flex min-h-screen max-w-2xl flex-col gap-6 p-10">
      <header>
        <h1 className="text-3xl font-black">Song Battle</h1>
        <p className="text-sm opacity-60">Music tournaments decided by your Kick chat.</p>
      </header>

      <Card>
        <div className="text-xs uppercase tracking-widest opacity-60">Rust IPC</div>
        <div className="mt-1 text-xl">{pong}</div>
      </Card>

      <Card>
        <div className="text-xs uppercase tracking-widest opacity-60">OBS overlay URL</div>
        <button
          type="button"
          onClick={() => overlay && openUrl(overlay)}
          className="mt-1 text-lg text-emerald-400 hover:underline"
        >
          {overlay || '—'}
        </button>
        <p className="mt-2 text-sm opacity-60">Add this as a Browser Source in OBS.</p>
      </Card>
    </main>
  );
}
