import { useState } from 'react';
import { Button, Field, Input } from '@sb/ui';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { PageHeader, Section, ErrorNote, EmptyState, Pill } from '../../components/common';

export function SongsPage() {
  const battle = useBattleStore((s) => s.snapshot?.battle ?? null);
  const { pending, error, run } = useAction();

  const [url, setUrl] = useState('');
  const [submitter, setSubmitter] = useState('');

  const songs = battle?.songs ?? [];
  const count = battle?.songCount ?? songs.length;

  const onImport = async () => {
    const ok = await run(() => ipc.importSong(url.trim(), submitter.trim() || undefined));
    if (ok) {
      setUrl('');
      setSubmitter('');
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Songs" subtitle="Import tracks by URL. YouTube, Spotify and SoundCloud links are supported." />

      <Section title="Import song">
        <p className="mb-4 text-sm text-white/40">
          Viewers can also add songs by typing <code className="rounded bg-white/10 px-1 text-white/70">!submit &lt;url&gt;</code> in
          chat while the bracket hasn&apos;t started yet (lobby only). Toggle this in Settings.
        </p>
        <div className="flex flex-col gap-4">
          <Field label="Source URL">
            <Input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://youtube.com/watch?v=…" spellCheck={false} />
          </Field>
          <Field label="Submitter" hint="Optional — who suggested it.">
            <Input value={submitter} onChange={(e) => setSubmitter(e.target.value)} placeholder="viewer name" />
          </Field>
          <ErrorNote message={error} />
          <div className="flex gap-3">
            <Button variant="primary" onClick={onImport} disabled={pending || url.trim().length === 0}>
              {pending ? 'Importing…' : 'Import'}
            </Button>
            <Button variant="secondary" onClick={() => run(() => ipc.shuffleSongs())} disabled={pending || count < 2}>
              Shuffle
            </Button>
          </div>
        </div>
      </Section>

      <Section title="Songs" action={<Pill tone="idle">{count} total</Pill>}>
        {songs.length === 0 ? (
          <EmptyState title="No songs yet" hint="Paste a track URL above to add the first contender." />
        ) : (
          <ul className="flex flex-col gap-2">
            {songs.map((song) => (
              <li
                key={song.id}
                className="flex items-center gap-3 rounded-xl border border-white/10 bg-black/20 p-2 pr-3"
              >
                <div className="h-12 w-12 shrink-0 overflow-hidden rounded-lg bg-white/5">
                  {song.thumbnail ? (
                    <img src={song.thumbnail} alt="" className="h-full w-full object-cover" />
                  ) : null}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-white">{song.title}</div>
                  <div className="truncate text-xs text-white/50">
                    {song.artist ? `${song.artist} · ` : ''}
                    {song.source}
                    {song.submitter ? ` · by ${song.submitter}` : ''}
                  </div>
                </div>
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => run(() => ipc.removeSong(song.id))}
                  disabled={pending}
                  aria-label={`Remove ${song.title}`}
                >
                  Remove
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}
