import { useEffect, useRef } from 'react';
import { Sidebar } from './features/nav/Sidebar';
import { PAGES, pageComponent, type PageId } from './features/nav/pages';
import { startSnapshotStream } from './stores/battle';
import { applyAccent } from './lib/settings';
import { usePersistentState } from './lib/usePersistentState';
import { useGlobalHotkeys } from './lib/useHotkeys';
import { useObsAutoSwitch } from './features/obs/useObs';
import { useDevRecorder } from './features/dev/useDevRecorder';
import { isTauri } from './lib/ipc';

export default function App() {
  const [page, setPage] = usePersistentState<PageId>('sb.page', 'home');
  const mainRef = useRef<HTMLElement>(null);

  useGlobalHotkeys();
  useObsAutoSwitch();
  useDevRecorder();

  // Sync with external systems: restore the saved accent token and subscribe to
  // the backend snapshot stream. Dispose the listener on unmount.
  useEffect(() => {
    if (!isTauri) return; // browser preview — no Tauri IPC to stream from
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

  // a11y: on page change, move focus to <main> and reflect the page in the title
  // (there's no router, so this is how a route change is announced).
  useEffect(() => {
    const label = PAGES.find((p) => p.id === page)?.label ?? 'Song Battle';
    document.title = `Song Battle — ${label}`;
    mainRef.current?.focus();
  }, [page]);

  const Page = pageComponent(page);

  // The dashboard needs the Tauri runtime (invoke/listen). Opened in a plain
  // browser it can't reach the backend — show how to launch the real app.
  if (!isTauri) return <BrowserNotice />;

  return (
    <div className="flex h-screen overflow-hidden">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-lg focus:bg-accent focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-black focus:shadow-xl"
      >
        Skip to content
      </a>
      <Sidebar current={page} onNavigate={setPage} />
      <main
        id="main-content"
        ref={mainRef}
        tabIndex={-1}
        className="flex-1 overflow-y-auto focus:outline-none"
      >
        <div className="mx-auto max-w-5xl px-8 py-8">
          <Page onNavigate={setPage} />
        </div>
      </main>
    </div>
  );
}

// Shown when the dashboard is opened in a web browser instead of the desktop app.
function BrowserNotice() {
  return (
    <div className="flex h-screen items-center justify-center bg-[#0a0a0f] p-8 text-center text-white">
      <div className="max-w-md">
        <h1 className="text-2xl font-black">Open the Song Battle desktop app</h1>
        <p className="mt-3 text-sm leading-relaxed text-white/60">
          You&apos;re viewing the dashboard in a web browser, which can&apos;t reach the app&apos;s
          backend (battles, voting, Kick, OBS). Launch the desktop window with{' '}
          <code className="rounded bg-white/10 px-1.5 py-0.5 font-mono text-emerald-400">
            pnpm dev
          </code>{' '}
          and use the window it opens.
        </p>
      </div>
    </div>
  );
}
