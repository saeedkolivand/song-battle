import { useRef, useState } from 'react';
import { Button, Select } from '@sb/ui';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { ACCENTS, getAccent, getTimerDefault, setAccent, setTimerDefault } from '../../lib/settings';
import { PageHeader, Section, ErrorNote } from '../../components/common';

const PRESETS = [10, 20, 30, 60];

export function SettingsPage() {
  const { pending, error, run } = useAction();
  const [accent, setAccentState] = useState(getAccent());
  const [timer, setTimer] = useState(getTimerDefault());
  const fileRef = useRef<HTMLInputElement>(null);

  const onAccent = (value: string) => {
    setAccentState(value);
    setAccent(value);
  };

  const onTimer = (value: number) => {
    setTimer(value);
    setTimerDefault(value);
  };

  const exportData = async () => {
    await run(async () => {
      const json = await ipc.exportJson();
      const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = 'song-battle.json';
      a.click();
      URL.revokeObjectURL(url);
    });
  };

  const importData = async (file: File) => {
    const text = await file.text();
    await run(() => ipc.importJson(text));
  };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Settings" subtitle="Renderer preferences and battle data." />

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

      <Section title="Default match timer">
        <label className="flex max-w-xs flex-col gap-1.5">
          <span className="text-xs font-medium uppercase tracking-wider text-white/50">Seconds</span>
          <Select value={String(timer)} onChange={(e) => onTimer(Number(e.target.value))}>
            {PRESETS.map((p) => (
              <option key={p} value={p}>
                {p}s
              </option>
            ))}
          </Select>
        </label>
        <p className="mt-2 text-sm text-white/40">Pre-fills the timer on the Bracket page.</p>
      </Section>

      <Section title="Battle data">
        <ErrorNote message={error} />
        <div className="mt-3 flex gap-3">
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
