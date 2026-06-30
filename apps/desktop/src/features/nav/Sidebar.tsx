import { useBattleStore } from '../../stores/battle';
import { PAGES, type PageId } from './pages';

export function Sidebar({ current, onNavigate }: { current: PageId; onNavigate: (id: PageId) => void }) {
  const live = useBattleStore((s) => s.live);
  const kickState = useBattleStore((s) => s.snapshot?.kick.state ?? 'disconnected');

  return (
    <nav aria-label="Primary navigation" className="flex h-screen w-56 shrink-0 flex-col border-r border-white/10 bg-black/40 p-3">
      <div className="px-3 py-4">
        <div className="text-sm font-black tracking-tight text-white">Song Battle</div>
        <div className="text-xs text-white/50">Kick edition</div>
      </div>

      <ul className="flex flex-1 flex-col gap-0.5">
        {PAGES.map((page) => {
          const active = page.id === current;
          return (
            <li key={page.id}>
              <button
                type="button"
                onClick={() => onNavigate(page.id)}
                aria-current={active ? 'page' : undefined}
                className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                  active ? 'bg-accent/15 font-medium text-accent' : 'text-white/60 hover:bg-white/5 hover:text-white'
                }`}
              >
                <span
                  className={`h-1.5 w-1.5 rounded-full ${active ? 'bg-accent' : 'bg-transparent'}`}
                  aria-hidden="true"
                />
                {page.label}
              </button>
            </li>
          );
        })}
      </ul>

      <div className="mt-2 flex flex-col gap-1 border-t border-white/10 px-3 pt-3 text-xs">
        <span className="flex items-center gap-2 text-white/50">
          <span className={`h-2 w-2 rounded-full ${live ? 'bg-emerald-400' : 'bg-amber-400'}`} aria-hidden="true" />
          {live ? 'backend live' : 'connecting…'}
        </span>
        <span className="text-white/50">kick: {kickState}</span>
      </div>
    </nav>
  );
}
