import { useEffect, useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '@sb/ui';
import { ipc } from '../../lib/ipc';
import { PageHeader, Section, ErrorNote, EmptyState } from '../../components/common';

export function OverlayPage() {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // One-shot config fetch from the backend (static URL) — sync, then render.
  useEffect(() => {
    let alive = true;
    ipc
      .overlayUrl()
      .then((u) => alive && setUrl(u))
      .catch((e) => alive && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      alive = false;
    };
  }, []);

  const copy = async () => {
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Overlay" subtitle="Add this URL as a Browser Source in OBS." />

      <Section title="OBS browser source">
        {error ? <ErrorNote message={error} /> : null}
        {url ? (
          <div className="flex flex-col gap-4">
            <code className="block overflow-x-auto rounded-xl border border-white/10 bg-black/40 px-4 py-3 font-mono text-sm text-accent">
              {url}
            </code>
            <div className="flex gap-3">
              <Button variant="primary" onClick={copy}>
                {copied ? 'Copied!' : 'Copy URL'}
              </Button>
              <Button variant="secondary" onClick={() => void openUrl(url)}>
                Open in browser
              </Button>
            </div>
            <p className="text-sm text-white/40">
              Width/height 1920×1080, transparent background. The overlay reconnects automatically.
            </p>
          </div>
        ) : !error ? (
          <EmptyState title="Loading overlay URL…" />
        ) : null}
      </Section>
    </div>
  );
}
