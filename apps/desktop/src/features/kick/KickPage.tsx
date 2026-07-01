import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button, Field, Input } from '@sb/ui';
import type { KickOfficialStatus, KickView } from '@sb/types';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import {
  PageHeader,
  Section,
  ConnectionPill,
  ErrorNote,
  Stat,
  Pill,
} from '../../components/common';

type Mode = 'unofficial' | 'official';

// Stable reference so the zustand selector below doesn't hand back a new object
// identity every render (which would loop useSyncExternalStore) before the
// first snapshot arrives.
const DISCONNECTED_KICK: KickView = { state: 'disconnected', channel: null };

export function KickPage() {
  const kick = useBattleStore((s) => s.snapshot?.kick ?? DISCONNECTED_KICK);
  const [mode, setMode] = useState<Mode>('unofficial');
  const unofficial = useAction();
  const [channel, setChannel] = useState('');
  const [chatroomId, setChatroomId] = useState('');

  const official = useAction();
  const [clientId, setClientId] = useState('');
  const [clientSecret, setClientSecret] = useState('');
  const [status, setStatus] = useState<KickOfficialStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);

  const refreshStatus = useCallback(() => {
    ipc
      .kickOfficialStatus()
      .then((s) => {
        setStatus(s);
        setStatusError(null);
      })
      .catch((e) => setStatusError(e instanceof Error ? e.message : String(e)));
  }, []);

  // Sync with the backend auth state: fetch on mount, then again whenever the
  // OAuth callback completes (the Rust loopback server emits `kick-auth` once
  // it has persisted tokens). Dispose the listener on unmount.
  useEffect(() => {
    refreshStatus();
    let active = true;
    let dispose: (() => void) | undefined;
    void listen('kick-auth', refreshStatus).then((unlisten) => {
      if (active) dispose = unlisten;
      else unlisten();
    });
    return () => {
      active = false;
      dispose?.();
    };
  }, [refreshStatus]);

  const busy = unofficial.pending || kick.state === 'connecting' || kick.state === 'reconnecting';
  const connected = kick.state === 'connected';
  const authorized = status?.authorized ?? false;
  const subscriptionActive = status?.subscriptionActive ?? false;

  const login = async () => {
    const ok = await official.run(async () => {
      const url = await ipc.kickOauthStart(clientId.trim(), clientSecret.trim());
      await openUrl(url);
    });
    if (ok) {
      setClientSecret(''); // backend has persisted it — don't keep the secret in component state
      refreshStatus();
    }
  };

  const disconnect = async () => {
    const ok = await official.run(() => ipc.kickOfficialDisconnect());
    if (ok) refreshStatus();
  };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Kick" subtitle="Connect to a channel's chat to collect !a / !b votes." />

      <div
        role="group"
        aria-label="Kick connection method"
        className="inline-flex w-fit gap-1 rounded-xl border border-white/10 bg-white/5 p-1"
      >
        <Button
          size="sm"
          variant={mode === 'unofficial' ? 'primary' : 'ghost'}
          aria-pressed={mode === 'unofficial'}
          onClick={() => setMode('unofficial')}
        >
          Unofficial
        </Button>
        <Button
          size="sm"
          variant={mode === 'official' ? 'primary' : 'ghost'}
          aria-pressed={mode === 'official'}
          onClick={() => setMode('official')}
        >
          Official
        </Button>
      </div>

      {mode === 'unofficial' ? (
        <Section title="Connection" action={<ConnectionPill state={kick.state} />}>
          <div className="flex flex-col gap-4">
            <Field label="Channel" hint="The Kick channel slug, e.g. xqc.">
              <Input
                value={channel}
                onChange={(e) => setChannel(e.target.value)}
                placeholder="channel-name"
                disabled={connected}
                autoCapitalize="none"
                spellCheck={false}
              />
            </Field>
            <Field label="Chatroom ID (optional)">
              <Input
                value={chatroomId}
                onChange={(e) => setChatroomId(e.target.value.replace(/[^0-9]/g, ''))}
                placeholder="e.g. 123456"
                inputMode="numeric"
                disabled={connected}
              />
            </Field>
            <p className="text-xs leading-relaxed text-white/50">
              Only needed if Connect is blocked by Kick (a &quot;security policy&quot; 403). Open
              this URL in your browser and copy <code className="text-white/70">chatroom.id</code>:{' '}
              <code className="break-all text-white/70">
                https://kick.com/api/v2/channels/{channel.trim() || 'your-channel'}
              </code>
            </p>
            <ErrorNote message={unofficial.error} />
            <div className="flex gap-3">
              <Button
                variant="primary"
                onClick={() =>
                  unofficial.run(() =>
                    ipc.connectKick(channel.trim(), chatroomId ? Number(chatroomId) : undefined),
                  )
                }
                disabled={busy || connected || channel.trim().length === 0}
              >
                {busy ? 'Connecting…' : 'Connect'}
              </Button>
              <Button
                variant="secondary"
                onClick={() => unofficial.run(() => ipc.disconnectKick())}
                disabled={unofficial.pending || kick.state === 'disconnected'}
              >
                Disconnect
              </Button>
            </div>
          </div>
        </Section>
      ) : (
        <Section
          title="Official Kick API"
          action={
            <Pill tone={authorized ? 'good' : 'idle'}>
              {authorized ? 'authorized' : 'not connected'}
            </Pill>
          }
        >
          <div className="flex flex-col gap-4">
            <Field label="Client ID">
              <Input
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder="Kick developer app client ID"
                disabled={authorized}
                autoCapitalize="none"
                spellCheck={false}
              />
            </Field>
            <Field label="Client Secret">
              <Input
                type="password"
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                placeholder="Kick developer app client secret"
                disabled={authorized}
                autoCapitalize="none"
                spellCheck={false}
              />
            </Field>
            <ErrorNote message={official.error ?? statusError} />
            <div className="flex gap-3">
              <Button
                variant="primary"
                onClick={() => void login()}
                disabled={
                  official.pending ||
                  authorized ||
                  clientId.trim().length === 0 ||
                  clientSecret.trim().length === 0
                }
              >
                {official.pending ? 'Opening…' : 'Login with Kick'}
              </Button>
              <Button
                variant="secondary"
                onClick={() => void disconnect()}
                disabled={official.pending || !authorized}
              >
                Disconnect
              </Button>
            </div>
            <div className="flex flex-col gap-2 rounded-lg border border-white/10 bg-white/5 p-3 text-xs leading-relaxed text-white/60">
              <p className="font-medium text-white/80">One-time setup at dev.kick.com</p>
              <ol className="list-decimal space-y-1 pl-4">
                <li>
                  Add redirect URI{' '}
                  <code className="rounded bg-black/40 px-1 text-white/80">
                    http://localhost:31337/oauth/callback
                  </code>
                </li>
                <li>
                  Expose{' '}
                  <code className="rounded bg-black/40 px-1 text-white/80">
                    http://localhost:31337/kick/webhook
                  </code>{' '}
                  over a public HTTPS tunnel and set that public URL as your app&apos;s Webhook URL.
                </li>
                <li>Log in below — chat votes then arrive over the webhook.</li>
              </ol>
              <p>
                Chat delivery:{' '}
                <span className={subscriptionActive ? 'text-emerald-400' : 'text-white/50'}>
                  {subscriptionActive ? 'subscribed' : 'not subscribed yet'}
                </span>
              </p>
            </div>
          </div>
        </Section>
      )}

      <Section title="Status">
        <div className="grid grid-cols-2 gap-3">
          <Stat label="State" value={kick.state} />
          <Stat label="Channel" value={kick.channel ?? '—'} />
        </div>
      </Section>
    </div>
  );
}
