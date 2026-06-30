import { useCallback, useEffect, useState } from 'react';
import { Button } from '@sb/ui';
import type { SavedBattle } from '@sb/types';
import { formatDateTime } from '@sb/shared';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import type { PageProps } from '../nav/pages';
import { PageHeader, Section, ErrorNote, EmptyState, BattlePill, Pill } from '../../components/common';

export function TournamentsPage({ onNavigate }: PageProps) {
  const activeId = useBattleStore((s) => s.snapshot?.battle?.id ?? null);
  const { pending, error, run } = useAction();
  const [battles, setBattles] = useState<SavedBattle[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setBattles(await ipc.listBattles());
    } finally {
      setLoading(false);
    }
  }, []);

  // Refetch on mount and whenever the active battle changes (load/delete/create).
  useEffect(() => {
    void refresh();
  }, [refresh, activeId]);

  const load = (id: string) =>
    run(async () => {
      await ipc.loadBattle(id);
      await refresh();
    });

  const remove = (id: string) =>
    run(async () => {
      await ipc.deleteBattle(id);
      await refresh();
    });

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Tournaments" subtitle="Saved battles. Load one to make it active." />

      <Section
        title="Saved battles"
        action={
          <Button variant="primary" size="sm" onClick={() => onNavigate('battle')} aria-label="New battle">
            New
          </Button>
        }
      >
        {error ? <ErrorNote message={error} /> : null}

        {loading && battles.length === 0 ? (
          <EmptyState title="Loading…" />
        ) : battles.length === 0 ? (
          <EmptyState title="No saved tournaments" hint="Create one from the Battle tab to get started." />
        ) : (
          <ul className="flex flex-col gap-2">
            {battles.map((b) => {
              const active = b.id === activeId;
              return (
                <li
                  key={b.id}
                  className={`flex items-center gap-3 rounded-xl border p-3 ${
                    active ? 'border-accent/40 bg-accent/5' : 'border-white/10 bg-black/20'
                  }`}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium text-white">{b.title || 'Untitled'}</span>
                      {active ? <Pill tone="live">active</Pill> : null}
                    </div>
                    <div className="mt-0.5 truncate text-xs text-white/50">
                      {b.theme ? `${b.theme} · ` : ''}
                      {b.songCount} song{b.songCount === 1 ? '' : 's'} · {formatDateTime(b.updatedAt)}
                    </div>
                  </div>
                  <BattlePill status={b.status} />
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => load(b.id)}
                    disabled={pending || active}
                    aria-label={`Load ${b.title || 'Untitled'}`}
                  >
                    Load
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => remove(b.id)}
                    disabled={pending}
                    aria-label={`Delete ${b.title}`}
                  >
                    Delete
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </Section>
    </div>
  );
}
