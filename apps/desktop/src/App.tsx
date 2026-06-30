import { useEffect, useState } from 'react';
import { Sidebar } from './features/nav/Sidebar';
import { pageComponent, type PageId } from './features/nav/pages';
import { startSnapshotStream } from './stores/battle';
import { applyAccent } from './lib/settings';
import { useGlobalHotkeys } from './lib/useHotkeys';

export default function App() {
  const [page, setPage] = useState<PageId>('home');

  useGlobalHotkeys();

  // Sync with external systems: restore the saved accent token and subscribe to
  // the backend snapshot stream. Dispose the listener on unmount.
  useEffect(() => {
    applyAccent();
    let dispose: (() => void) | undefined;
    let active = true;
    void startSnapshotStream().then((unlisten) => {
      if (active) dispose = unlisten;
      else unlisten();
    });
    return () => {
      active = false;
      dispose?.();
    };
  }, []);

  const Page = pageComponent(page);

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar current={page} onNavigate={setPage} />
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-8 py-8">
          <Page onNavigate={setPage} />
        </div>
      </main>
    </div>
  );
}
