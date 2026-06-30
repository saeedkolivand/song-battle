import { useBattleStore } from '../../stores/battle';
import { PageHeader, Section, Stat, BattlePill, ConnectionPill, EmptyState, Pill } from '../../components/common';

export function HomePage() {
  const snapshot = useBattleStore((s) => s.snapshot);
  const live = useBattleStore((s) => s.live);
  const battle = snapshot?.battle ?? null;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Song Battle" subtitle="Music tournaments decided by your Kick chat." />

      <Section
        title="Backend"
        action={<Pill tone={live ? 'good' : 'warn'}>{live ? 'snapshot stream live' : 'waiting for backend'}</Pill>}
      >
        <p className="text-sm text-white/50">
          The dashboard mirrors the backend snapshot. Stats below update automatically as the battle runs.
        </p>
      </Section>

      {battle ? (
        <Section title="Current battle" action={<BattlePill status={battle.status} />}>
          <div className="mb-4">
            <div className="text-lg font-semibold text-white">{battle.title || 'Untitled battle'}</div>
            {battle.theme ? <div className="text-sm text-white/50">{battle.theme}</div> : null}
          </div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Stat label="Songs" value={battle.songCount} />
            <Stat label="Round" value={`${battle.round} / ${battle.totalRounds}`} />
            <Stat label="Matches" value={battle.bracket.length} />
            <Stat label="Winner" value={battle.winner?.title ?? '—'} />
          </div>
        </Section>
      ) : (
        <EmptyState title="No battle yet" hint="Head to the Battle tab to create one, then import songs and generate a bracket." />
      )}

      <Section title="Kick" action={<ConnectionPill state={snapshot?.kick.state ?? 'disconnected'} />}>
        <p className="text-sm text-white/50">
          Channel: <span className="text-white/80">{snapshot?.kick.channel ?? '—'}</span>
        </p>
      </Section>
    </div>
  );
}
