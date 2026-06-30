import { useState } from 'react';
import { Button, Field, Input } from '@sb/ui';
import { useBattleStore } from '../../stores/battle';
import { ipc } from '../../lib/ipc';
import { useAction } from '../../lib/useAction';
import { PageHeader, Section, ConnectionPill, ErrorNote, Stat } from '../../components/common';

export function KickPage() {
  const kick = useBattleStore(
    (s) => s.snapshot?.kick ?? { state: 'disconnected' as const, channel: null },
  );
  const { pending, error, run } = useAction();
  const [channel, setChannel] = useState('');
  const [chatroomId, setChatroomId] = useState('');

  const busy = pending || kick.state === 'connecting' || kick.state === 'reconnecting';
  const connected = kick.state === 'connected';

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Kick" subtitle="Connect to a channel's chat to collect !a / !b votes." />

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
            Only needed if Connect is blocked by Kick (a &quot;security policy&quot; 403). Open this
            URL in your browser and copy <code className="text-white/70">chatroom.id</code>:{' '}
            <code className="break-all text-white/70">
              https://kick.com/api/v2/channels/{channel.trim() || 'your-channel'}
            </code>
          </p>
          <ErrorNote message={error} />
          <div className="flex gap-3">
            <Button
              variant="primary"
              onClick={() =>
                run(() =>
                  ipc.connectKick(channel.trim(), chatroomId ? Number(chatroomId) : undefined),
                )
              }
              disabled={busy || connected || channel.trim().length === 0}
            >
              {busy ? 'Connecting…' : 'Connect'}
            </Button>
            <Button
              variant="secondary"
              onClick={() => run(() => ipc.disconnectKick())}
              disabled={pending || kick.state === 'disconnected'}
            >
              Disconnect
            </Button>
          </div>
        </div>
      </Section>

      <Section title="Status">
        <div className="grid grid-cols-2 gap-3">
          <Stat label="State" value={kick.state} />
          <Stat label="Channel" value={kick.channel ?? '—'} />
        </div>
      </Section>
    </div>
  );
}
