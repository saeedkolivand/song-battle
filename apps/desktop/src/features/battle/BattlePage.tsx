import { useState } from 'react';
import { Button, Field, Input, TextArea } from '@sb/ui';
import { modeLabel } from '@sb/shared';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { PageHeader, Section, Stat, BattlePill, ErrorNote } from '../../components/common';

export function BattlePage() {
  const battle = useBattleStore((s) => s.snapshot?.battle ?? null);
  const { pending, error, run } = useAction();

  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [theme, setTheme] = useState('');

  const onCreate = async () => {
    const ok = await run(() => ipc.createBattle(title.trim(), description.trim(), theme.trim()));
    if (ok) {
      setTitle('');
      setDescription('');
      setTheme('');
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Battle" subtitle="Create a battle, then add songs and generate a bracket." />

      <Section title="New battle">
        <div className="flex flex-col gap-4">
          <Field label="Title">
            <Input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Best 2000s Bangers" />
          </Field>
          <Field label="Description" hint="Optional — shown to your audience.">
            <TextArea value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Bracket of all-time favourites" />
          </Field>
          <Field label="Theme" hint="Optional — e.g. a genre or decade.">
            <Input value={theme} onChange={(e) => setTheme(e.target.value)} placeholder="2000s" />
          </Field>
          <ErrorNote message={error} />
          <div>
            <Button variant="primary" onClick={onCreate} disabled={pending || title.trim().length === 0}>
              {pending ? 'Creating…' : 'Create battle'}
            </Button>
          </div>
        </div>
      </Section>

      {battle ? (
        <Section title="Current battle" action={<BattlePill status={battle.status} />}>
          <div className="mb-4">
            <div className="text-lg font-semibold text-white">{battle.title || 'Untitled battle'}</div>
            {battle.description ? <p className="mt-1 text-sm text-white/60">{battle.description}</p> : null}
          </div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Stat label="Mode" value={modeLabel(battle.mode)} />
            <Stat label="Theme" value={battle.theme || '—'} />
            <Stat label="Songs" value={battle.songCount} />
            <Stat label="Round" value={`${battle.round} / ${battle.totalRounds}`} />
            <Stat label="Status" value={battle.status} />
          </div>
        </Section>
      ) : null}
    </div>
  );
}
