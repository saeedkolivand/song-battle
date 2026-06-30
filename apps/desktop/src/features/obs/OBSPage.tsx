import { Button, Field, Input, Select } from '@sb/ui';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { PageHeader, Section, ConnectionPill, ErrorNote } from '../../components/common';
import { useObsStore } from './useObs';
import type { ObsScenes } from './obsSettings';

function SceneField({
  label,
  value,
  connected,
  scenes,
  onChange,
}: {
  label: string;
  value: string;
  connected: boolean;
  scenes: string[];
  onChange: (value: string) => void;
}) {
  if (connected && scenes.length > 0) {
    return (
      <Field label={label}>
        <Select value={value} onChange={(e) => onChange(e.target.value)}>
          <option value="">— none —</option>
          {value && !scenes.includes(value) ? <option value={value}>{value} (not found)</option> : null}
          {scenes.map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </Select>
      </Field>
    );
  }
  return (
    <Field label={label}>
      <Input value={value} onChange={(e) => onChange(e.target.value)} placeholder="Scene name" spellCheck={false} />
    </Field>
  );
}

export function OBSPage() {
  const { pending, error, run } = useAction();
  const state = useObsStore((s) => s.state);
  const obsError = useObsStore((s) => s.error);
  const settings = useObsStore((s) => s.settings);
  const scenes = useObsStore((s) => s.scenes);
  const patchSettings = useObsStore((s) => s.patchSettings);
  const setScene = useObsStore((s) => s.setScene);
  const connect = useObsStore((s) => s.connect);
  const disconnect = useObsStore((s) => s.disconnect);
  const switchScene = useObsStore((s) => s.switchScene);
  const setBrowserSourceUrl = useObsStore((s) => s.setBrowserSourceUrl);

  const connected = state === 'connected';
  const busy = pending || state === 'connecting';

  const sceneEntries: { key: keyof ObsScenes; label: string }[] = [
    { key: 'battle', label: 'Battle scene' },
    { key: 'winner', label: 'Winner scene' },
    { key: 'intermission', label: 'Intermission scene' },
  ];

  const setOverlaySource = () =>
    run(async () => {
      const url = await ipc.overlayUrl();
      await setBrowserSourceUrl(settings.browserSourceName.trim(), url);
    });

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="OBS" subtitle="Drive OBS scenes and the overlay source over the OBS WebSocket." />

      <Section title="Connection" action={<ConnectionPill state={state} />}>
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label="WebSocket URL" hint="OBS → Tools → WebSocket Server Settings.">
              <Input
                value={settings.url}
                onChange={(e) => patchSettings({ url: e.target.value })}
                placeholder="ws://127.0.0.1:4455"
                disabled={connected}
                spellCheck={false}
              />
            </Field>
            <Field label="Password" hint="Stored locally in plaintext.">
              <Input
                type="password"
                value={settings.password}
                onChange={(e) => patchSettings({ password: e.target.value })}
                placeholder="(if Authentication is enabled)"
                disabled={connected}
              />
            </Field>
          </div>
          <ErrorNote message={error ?? obsError} />
          <div className="flex gap-3">
            <Button
              variant="primary"
              onClick={() => run(() => connect())}
              disabled={busy || connected || settings.url.trim().length === 0}
            >
              {state === 'connecting' ? 'Connecting…' : 'Connect'}
            </Button>
            <Button variant="secondary" onClick={() => run(() => disconnect())} disabled={pending || state === 'disconnected' || state === 'connecting'}>
              Disconnect
            </Button>
          </div>
        </div>
      </Section>

      <Section title="Scene mapping">
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            {sceneEntries.map(({ key, label }) => (
              <SceneField
                key={key}
                label={label}
                value={settings.scenes[key]}
                connected={connected}
                scenes={scenes}
                onChange={(v) => setScene(key, v)}
              />
            ))}
          </div>

          <label className="flex cursor-pointer items-center gap-3">
            <span className="relative inline-flex shrink-0">
              <input
                type="checkbox"
                className="peer sr-only"
                checked={settings.autoSwitch}
                onChange={(e) => patchSettings({ autoSwitch: e.target.checked })}
              />
              <span className="sb-toggle-track h-6 w-11 rounded-full bg-white/15 transition-colors peer-checked:bg-accent peer-focus-visible:outline-none peer-focus-visible:ring-2 peer-focus-visible:ring-accent peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-black" />
              <span className="sb-toggle-thumb pointer-events-none absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white transition-transform peer-checked:translate-x-5" />
            </span>
            <span className="text-sm text-white">Auto-switch scenes from battle state</span>
          </label>
          <p className="text-sm text-white/50">
            Active match → Battle, champion crowned → Winner, otherwise → Intermission.
          </p>
        </div>
      </Section>

      <Section title="Manual scene switch">
        <div className="flex flex-wrap gap-3">
          {sceneEntries.map(({ key, label }) => (
            <Button
              key={key}
              variant="secondary"
              onClick={() => run(() => switchScene(settings.scenes[key]))}
              disabled={!connected || pending || settings.scenes[key].trim().length === 0}
            >
              {label.replace(' scene', '')}
            </Button>
          ))}
        </div>
      </Section>

      <Section title="Overlay browser source">
        <div className="flex flex-col gap-4">
          <Field label="Browser source name" hint="The exact name of the Browser source in OBS.">
            <Input
              value={settings.browserSourceName}
              onChange={(e) => patchSettings({ browserSourceName: e.target.value })}
              placeholder="Song Battle Overlay"
              spellCheck={false}
            />
          </Field>
          <div>
            <Button
              variant="secondary"
              onClick={setOverlaySource}
              disabled={!connected || pending || settings.browserSourceName.trim().length === 0}
            >
              Set browser source to overlay URL
            </Button>
          </div>
        </div>
      </Section>
    </div>
  );
}
