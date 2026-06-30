import { useEffect, useRef, useState } from 'react';
import { Button, Select } from '@sb/ui';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { ACCENTS, getAccent, setAccent } from '../../lib/settings';
import { HOTKEYS } from '../../lib/useHotkeys';
import { PageHeader, Section, ErrorNote } from '../../components/common';

const PRESETS = [10, 20, 30, 60];

export function SettingsPage() {
  const { pending, error, run } = useAction();
  const [accent, setAccentState] = useState(getAccent());
  const [anonymous, setAnonymous] = useState(false);
  const [timer, setTimer] = useState(30);
  const fileRef = useRef<HTMLInputElement>(null);

  // Seed persisted settings from the backend (one-shot on mount).
  useEffect(() => {
    let alive = true;
    ipc
      .getSettings()
      .then((s) => {
        if (!alive) return;
        setAnonymous(s.anonymous);
        setTimer(s.defaultTimerSec);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const onAccent = (value: string) => {
    setAccentState(value);
    setAccent(value);
  };

  // Persist first, reflect after success (no optimistic flip before the IPC resolves).
  const onAnonymous = (value: boolean) =>
    run(async () => {
      await ipc.setAnonymous(value);
      setAnonymous(value);
    });

  const onTimer = (value: number) =>
    run(async () => {
      await ipc.setDefaultTimer(value);
      setTimer(value);
    });

  const exportData = () =>
    run(async () => {
      const json = await ipc.exportJson();
      const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = 'song-battle.json';
      a.click();
      URL.revokeObjectURL(url);
    });

  const importData = (file: File) => run(async () => ipc.importJson(await file.text()));

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Settings" subtitle="Preferences, voting privacy, and battle data." />

      {error ? <ErrorNote message={error} /> : null}

      <Section title="Voting">
        <label className="flex cursor-pointer items-center gap-3">
          <span className="relative inline-flex shrink-0">
            <input
              type="checkbox"
              className="peer sr-only"
              checked={anonymous}
              onChange={(e) => onAnonymous(e.target.checked)}
              disabled={pending}
            />
            <span className="h-6 w-11 rounded-full bg-white/15 transition-colors peer-checked:bg-accent peer-focus-visible:outline-none peer-focus-visible:ring-2 peer-focus-visible:ring-accent peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-black" />
            <span className="pointer-events-none absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white transition-transform peer-checked:translate-x-5" />
          </span>
          <span className="text-sm text-white">Anonymous voting</span>
        </label>
        <p className="mt-2 text-sm text-white/40">Hides voter identities everywhere. Only counts and totals are shown.</p>
      </Section>

      <Section title="Default match timer">
        <label className="flex max-w-xs flex-col gap-1.5">
          <span className="text-xs font-medium uppercase tracking-wider text-white/50">Seconds</span>
          <Select value={String(timer)} onChange={(e) => onTimer(Number(e.target.value))} disabled={pending}>
            {PRESETS.map((p) => (
              <option key={p} value={p}>
                {p}s
              </option>
            ))}
          </Select>
        </label>
        <p className="mt-2 text-sm text-white/40">Used by Start match when no per-match timer is set.</p>
      </Section>

      <Section title="Accent colour">
        <div className="flex flex-wrap gap-3">
          {ACCENTS.map((a) => (
            <button
              key={a.value}
              type="button"
              onClick={() => onAccent(a.value)}
              aria-label={a.name}
              aria-pressed={accent === a.value}
              className={`h-10 w-10 rounded-full border-2 transition-transform focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white focus-visible:ring-offset-2 focus-visible:ring-offset-black ${
                accent === a.value ? 'scale-110 border-white' : 'border-transparent hover:scale-105'
              }`}
              style={{ backgroundColor: a.value }}
            />
          ))}
        </div>
      </Section>

      <Section title="Keyboard shortcuts">
        <p className="mb-3 text-sm text-white/40">Active in the dashboard (ignored while typing in a field).</p>
        <dl className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {HOTKEYS.map((h) => (
            <div key={h.keys} className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-black/20 px-3 py-2">
              <dt className="text-sm text-white/70">{h.label}</dt>
              <dd>
                <kbd className="rounded-md border border-white/20 bg-white/10 px-2 py-0.5 text-xs font-semibold text-white">
                  {h.keys}
                </kbd>
              </dd>
            </div>
          ))}
        </dl>
      </Section>

      <Section title="Battle data">
        <div className="flex gap-3">
          <Button variant="secondary" onClick={exportData} disabled={pending}>
            Export JSON
          </Button>
          <Button variant="secondary" onClick={() => fileRef.current?.click()} disabled={pending}>
            Import JSON
          </Button>
          <input
            ref={fileRef}
            type="file"
            accept="application/json"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) void importData(file);
              e.target.value = '';
            }}
          />
        </div>
      </Section>
    </div>
  );
}
