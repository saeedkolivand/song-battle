import type { ComponentType } from 'react';
import { HomePage } from '../home/HomePage';
import { BattlePage } from '../battle/BattlePage';
import { TournamentsPage } from '../tournaments/TournamentsPage';
import { SongsPage } from '../songs/SongsPage';
import { BracketPage } from '../bracket/BracketPage';
import { KickPage } from '../kick/KickPage';
import { OverlayPage } from '../overlay/OverlayPage';
import { OBSPage } from '../obs/OBSPage';
import { SettingsPage } from '../settings/SettingsPage';
import { DevPanelPage } from '../dev/DevPanelPage';
import { AboutPage } from '../about/AboutPage';

export type PageId =
  | 'home'
  | 'battle'
  | 'tournaments'
  | 'songs'
  | 'bracket'
  | 'kick'
  | 'overlay'
  | 'obs'
  | 'settings'
  | 'logs'
  | 'about';

// Pages may navigate (e.g. Tournaments → Battle). Most ignore the prop.
export interface PageProps {
  onNavigate: (id: PageId) => void;
}

export interface PageDef {
  id: PageId;
  label: string;
  component: ComponentType<PageProps>;
}

// Ordered for the sidebar. Functional pages first, utility/placeholder last.
export const PAGES: PageDef[] = [
  { id: 'home', label: 'Home', component: HomePage },
  { id: 'battle', label: 'Battle', component: BattlePage },
  { id: 'tournaments', label: 'Tournaments', component: TournamentsPage },
  { id: 'songs', label: 'Songs', component: SongsPage },
  { id: 'bracket', label: 'Bracket', component: BracketPage },
  { id: 'kick', label: 'Kick', component: KickPage },
  { id: 'overlay', label: 'Overlay', component: OverlayPage },
  { id: 'obs', label: 'OBS', component: OBSPage },
  { id: 'settings', label: 'Settings', component: SettingsPage },
  { id: 'logs', label: 'Dev', component: DevPanelPage },
  { id: 'about', label: 'About', component: AboutPage },
];

export function pageComponent(id: PageId): ComponentType<PageProps> {
  return (PAGES.find((p) => p.id === id) ?? PAGES[0]).component;
}
