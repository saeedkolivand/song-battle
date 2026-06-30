import { useEffect, useState } from 'react';
import { Button, Select } from '@sb/ui';
import type { BattleView, BattleMode, MatchGroup, MatchView } from '@sb/types';
import { modeLabel, groupLabel } from '@sb/shared';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { PageHeader, Section, ErrorNote, EmptyState, MatchPill, SeriesScore } from '../../components/common';
import { LiveMatch } from './LiveMatch';

const PRESETS = [10, 20, 30, 60];
const MODE_OPTIONS: BattleMode[] = ['single', 'double', 'bo3'];
const DOUBLE_GROUPS: MatchGroup[] = ['winners', 'losers', 'grand'];

function groupByRound(bracket: MatchView[]): [number, MatchView[]][] {
  const rounds = new Map<number, MatchView[]>();
  for (const m of bracket) {
    const list = rounds.get(m.round) ?? [];
    list.push(m);
    rounds.set(m.round, list);
  }
  return [...rounds.entries()].sort((a, b) => a[0] - b[0]);
}

function MiniMatch({ match }: { match: MatchView }) {
  const aWon = match.winner === 'a';
  const bWon = match.winner === 'b';
  const series = match.bestOf > 1;
  return (
    <div className="rounded-xl border border-white/10 bg-black/20 p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs text-white/40">{series ? `Best of ${match.bestOf}` : 'Match'}</span>
        <MatchPill state={match.state} />
      </div>
      <div className="flex flex-col gap-1 text-sm">
        <div className={`flex justify-between gap-2 ${aWon ? 'text-accent' : 'text-white/80'}`}>
          <span className="truncate">{match.a?.title ?? '—'}</span>
          <span className="tabular-nums">{series ? match.winsA : match.votesA}</span>
        </div>
        <div className={`flex justify-between gap-2 ${bWon ? 'text-accent' : 'text-white/80'}`}>
          <span className="truncate">{match.b?.title ?? '—'}</span>
          <span className="tabular-nums">{series ? match.winsB : match.votesB}</span>
        </div>
      </div>
      {series ? (
        <div className="mt-2 flex justify-center">
          <SeriesScore winsA={match.winsA} winsB={match.winsB} bestOf={match.bestOf} />
        </div>
      ) : null}
    </div>
  );
}

function RoundsGrid({ matches }: { matches: MatchView[] }) {
  return (
    <div className="flex flex-col gap-5">
      {groupByRound(matches).map(([round, ms]) => (
        <div key={round}>
          <div className="mb-2 text-xs uppercase tracking-wider text-white/40">
            Round {round} · {ms.length} match{ms.length === 1 ? '' : 'es'}
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {ms.map((m) => (
              <MiniMatch key={m.id} match={m} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

// Double-elim → Winners / Losers / Grand sections (each sub-grouped by round).
// Single / bo3 → a flat rounds layout.
function BracketView({ battle }: { battle: BattleView }) {
  if (battle.mode !== 'double') {
    return <RoundsGrid matches={battle.bracket} />;
  }
  return (
    <div className="flex flex-col gap-6">
      {DOUBLE_GROUPS.map((g) => {
        const matches = battle.bracket.filter((m) => m.group === g);
        if (matches.length === 0) return null;
        return (
          <div key={g}>
            <div className="mb-2 text-sm font-semibold uppercase tracking-wider text-white/60">{groupLabel(g)}</div>
            <RoundsGrid matches={matches} />
          </div>
        );
      })}
    </div>
  );
}

export function BracketPage() {
  const battle = useBattleStore((s) => s.snapshot?.battle ?? null);
  const anonymous = useBattleStore((s) => s.snapshot?.anonymous ?? false);
  const { pending, error, run } = useAction();

  const [preset, setPreset] = useState<string>('30');
  const [custom, setCustom] = useState<number>(30);
  const [mode, setMode] = useState<BattleMode>('single');

  // Seed the timer control from the persisted default (one-shot on mount).
  useEffect(() => {
    let alive = true;
    ipc
      .getSettings()
      .then((s) => {
        if (!alive) return;
        setPreset(PRESETS.includes(s.defaultTimerSec) ? String(s.defaultTimerSec) : 'custom');
        setCustom(s.defaultTimerSec);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const seconds = preset === 'custom' ? custom : Number(preset);
  const current = battle?.currentMatch ?? null;

  const applyTimer = async () => {
    if (!Number.isFinite(seconds) || seconds <= 0) return;
    await run(() => ipc.setTimer(seconds));
  };

  if (!battle) {
    return (
      <div className="flex flex-col gap-6">
        <PageHeader title="Bracket" />
        <EmptyState title="No battle yet" hint="Create a battle and import songs before generating a bracket." />
      </div>
    );
  }

  const hasBracket = battle.bracket.length > 0;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Bracket" subtitle={`${modeLabel(battle.mode)} · run matches and let chat decide.`} />

      {error ? <ErrorNote message={error} /> : null}

      {!hasBracket ? (
        <Section title="Generate bracket">
          <p className="mb-4 text-sm text-white/50">
            {battle.songCount} song{battle.songCount === 1 ? '' : 's'} imported. You need at least 2 to build a bracket.
          </p>
          <div className="flex flex-wrap items-end gap-3">
            <label className="flex flex-col gap-1.5">
              <span className="text-xs font-medium uppercase tracking-wider text-white/50">Format</span>
              <Select value={mode} onChange={(e) => setMode(e.target.value as BattleMode)} className="w-56">
                {MODE_OPTIONS.map((m) => (
                  <option key={m} value={m}>
                    {modeLabel(m)}
                  </option>
                ))}
              </Select>
            </label>
            <Button
              variant="primary"
              onClick={() => run(() => ipc.generateBracket(mode))}
              disabled={pending || battle.songCount < 2}
            >
              {pending ? 'Generating…' : 'Generate bracket'}
            </Button>
          </div>
        </Section>
      ) : (
        <>
          <Section title="Match controls">
            <div className="flex flex-col gap-4">
              <div className="flex flex-wrap items-end gap-3">
                <label className="flex flex-col gap-1.5">
                  <span className="text-xs font-medium uppercase tracking-wider text-white/50">Timer</span>
                  <Select value={preset} onChange={(e) => setPreset(e.target.value)} className="w-36">
                    {PRESETS.map((p) => (
                      <option key={p} value={p}>
                        {p}s
                      </option>
                    ))}
                    <option value="custom">Custom…</option>
                  </Select>
                </label>
                {preset === 'custom' ? (
                  <label className="flex flex-col gap-1.5">
                    <span className="text-xs font-medium uppercase tracking-wider text-white/50">Seconds</span>
                    <input
                      type="number"
                      min={1}
                      value={custom}
                      onChange={(e) => setCustom(Number(e.target.value))}
                      className="h-10 w-28 rounded-xl border border-white/15 bg-black/30 px-3 text-sm text-white focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/40"
                    />
                  </label>
                ) : null}
                <Button variant="secondary" onClick={applyTimer} disabled={pending || seconds <= 0}>
                  Set timer
                </Button>
              </div>

              <div className="flex flex-wrap gap-3">
                <Button
                  variant="primary"
                  onClick={() => run(() => ipc.startMatch())}
                  disabled={pending || !current || current.state === 'active'}
                >
                  Start match
                </Button>
                <Button variant="secondary" onClick={() => run(() => ipc.resetVotes())} disabled={pending || !current}>
                  Reset votes
                </Button>
                <Button variant="secondary" onClick={() => run(() => ipc.skipMatch())} disabled={pending || !current}>
                  Skip match
                </Button>
              </div>
            </div>
          </Section>

          <Section title="Current match">
            {current ? (
              <LiveMatch match={current} anonymous={anonymous} />
            ) : battle.winner ? (
              <EmptyState title={`Winner: ${battle.winner.title}`} hint="The battle has finished." />
            ) : (
              <EmptyState title="No active match" hint="Start a match to begin voting." />
            )}
          </Section>

          <Section title="Bracket">
            <BracketView battle={battle} />
          </Section>
        </>
      )}
    </div>
  );
}
