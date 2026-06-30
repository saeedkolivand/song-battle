import { useEffect, useRef, useState } from 'react';
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import type { Announcements, DragEndEvent, UniqueIdentifier } from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Button, Field, Input } from '@sb/ui';
import type { Song } from '@sb/types';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { PageHeader, Section, ErrorNote, EmptyState, Pill } from '../../components/common';

function GripIcon() {
  return (
    <svg viewBox="0 0 10 16" width="11" height="16" aria-hidden="true" className="fill-current">
      <circle cx="2" cy="3" r="1.3" />
      <circle cx="8" cy="3" r="1.3" />
      <circle cx="2" cy="8" r="1.3" />
      <circle cx="8" cy="8" r="1.3" />
      <circle cx="2" cy="13" r="1.3" />
      <circle cx="8" cy="13" r="1.3" />
    </svg>
  );
}

// Shared row content (seed, artwork, metadata, remove) used by both the sortable
// and the locked/static list.
function RowBody({ song, seed, onRemove, disabled }: { song: Song; seed: number; onRemove: () => void; disabled: boolean }) {
  return (
    <>
      <span className="w-8 shrink-0 text-center text-sm font-bold tabular-nums text-white/50">#{seed}</span>
      <div className="h-12 w-12 shrink-0 overflow-hidden rounded-lg bg-white/5">
        {song.thumbnail ? <img src={song.thumbnail} alt="" className="h-full w-full object-cover" /> : null}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium text-white">{song.title}</div>
        <div className="truncate text-xs text-white/50">
          {song.artist ? `${song.artist} · ` : ''}
          {song.source}
          {song.submitter ? ` · by ${song.submitter}` : ''}
        </div>
      </div>
      <Button size="sm" variant="danger" onClick={onRemove} disabled={disabled} aria-label={`Remove ${song.title}`}>
        Remove
      </Button>
    </>
  );
}

function SortableSongRow({
  song,
  seed,
  onRemove,
  disabled,
}: {
  song: Song;
  seed: number;
  onRemove: () => void;
  disabled: boolean;
}) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({
    id: song.id,
  });

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={`flex items-center gap-3 rounded-xl border border-white/10 bg-black/20 p-2 pr-3 ${
        isDragging ? 'relative z-10 border-accent/40 shadow-xl' : ''
      }`}
    >
      <button
        type="button"
        ref={setActivatorNodeRef}
        {...attributes}
        {...listeners}
        disabled={disabled}
        aria-label={`Reorder ${song.title}`}
        className="flex h-9 w-7 shrink-0 cursor-grab touch-none items-center justify-center rounded-md text-white/50 hover:text-white/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent active:cursor-grabbing"
      >
        <GripIcon />
      </button>
      <RowBody song={song} seed={seed} onRemove={onRemove} disabled={disabled} />
    </li>
  );
}

export function SongsPage() {
  const battle = useBattleStore((s) => s.snapshot?.battle ?? null);
  const { pending, error, run } = useAction();

  const [url, setUrl] = useState('');
  const [submitter, setSubmitter] = useState('');

  const songs = battle?.songs ?? [];
  const count = battle?.songCount ?? songs.length;
  const lobby = battle !== null && battle.bracket.length === 0 && battle.currentMatch === null;

  // Local order = optimistic mirror of battle.songs; reconcile when the snapshot's
  // song membership/order actually changes (keyed by id signature, avoids flicker on
  // unrelated ~10/s snapshot ticks). A ref keeps the latest songs without making the
  // effect depend on the array reference.
  const sig = songs.map((s) => s.id).join(',');
  const songsRef = useRef(songs);
  songsRef.current = songs;
  const [items, setItems] = useState<Song[]>(songs);
  useEffect(() => {
    setItems(songsRef.current);
  }, [sig]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const onDragEnd = (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const oldIndex = items.findIndex((s) => s.id === active.id);
    const newIndex = items.findIndex((s) => s.id === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    const next = arrayMove(items, oldIndex, newIndex);
    setItems(next); // optimistic
    void run(() => ipc.reorderSongs(next.map((s) => s.id)));
  };

  // Screen-reader live-region text during a (keyboard) drag — song titles, not UUIDs.
  const titleOf = (id: UniqueIdentifier) => items.find((s) => s.id === id)?.title ?? String(id);
  const announcements: Announcements = {
    onDragStart: ({ active }) => `Picked up ${titleOf(active.id)}.`,
    onDragOver: ({ active, over }) =>
      over ? `${titleOf(active.id)} is now over ${titleOf(over.id)}.` : undefined,
    onDragEnd: ({ active, over }) =>
      over
        ? `${titleOf(active.id)} was dropped over ${titleOf(over.id)}.`
        : `Reorder of ${titleOf(active.id)} cancelled.`,
    onDragCancel: ({ active }) => `Reorder of ${titleOf(active.id)} cancelled.`,
  };

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
        <p className="mb-4 text-sm text-white/50">
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
        <p className="mb-4 text-sm text-white/50">
          {lobby
            ? 'Drag to set the bracket seeding (#1 faces the lowest seed).'
            : 'Seeding locked — a bracket has been generated.'}
        </p>

        {items.length === 0 ? (
          <EmptyState title="No songs yet" hint="Paste a track URL above to add the first contender." />
        ) : lobby ? (
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={onDragEnd}
            accessibility={{ announcements }}
          >
            <SortableContext items={items.map((s) => s.id)} strategy={verticalListSortingStrategy}>
              <ul className="flex flex-col gap-2">
                {items.map((song, i) => (
                  <SortableSongRow
                    key={song.id}
                    song={song}
                    seed={i + 1}
                    disabled={pending}
                    onRemove={() => run(() => ipc.removeSong(song.id))}
                  />
                ))}
              </ul>
            </SortableContext>
          </DndContext>
        ) : (
          <ul className="flex flex-col gap-2">
            {items.map((song, i) => (
              <li key={song.id} className="flex items-center gap-3 rounded-xl border border-white/10 bg-black/20 p-2 pr-3">
                <RowBody song={song} seed={i + 1} onRemove={() => run(() => ipc.removeSong(song.id))} disabled={pending} />
              </li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}
