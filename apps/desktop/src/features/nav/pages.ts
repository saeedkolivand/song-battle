import type { ComponentType } from 'react';
import { HomePage } from '../home/HomePage';
import { BattlePage } from '../battle/BattlePage';
import { SongsPage } from '../songs/SongsPage';
import { BracketPage } from '../bracket/BracketPage';
import { KickPage } from '../kick/KickPage';
import { OverlayPage } from '../overlay/OverlayPage';
import { SettingsPage } from '../settings/SettingsPage';
import { LogsPage } from '../logs/LogsPage';
import { AboutPage } from '../about/AboutPage';

export type PageId =
  | 'home'
  | 'battle'
  | 'songs'
  | 'bracket'
  | 'kick'
  | 'overlay'
  | 'settings'
  | 'logs'
  | 'about';

export interface PageDef {
  id: PageId;
  label: string;
  component: ComponentType;
}

// Ordered for the sidebar. `functional` pages first, utility/placeholder last.
export const PAGES: PageDef[] = [
  { id: 'home', label: 'Home', component: HomePage },
  { id: 'battle', label: 'Battle', component: BattlePage },
  { id: 'songs', label: 'Songs', component: SongsPage },
  { id: 'bracket', label: 'Bracket', component: BracketPage },
  { id: 'kick', label: 'Kick', component: KickPage },
  { id: 'overlay', label: 'Overlay', component: OverlayPage },
  { id: 'settings', label: 'Settings', component: SettingsPage },
  { id: 'logs', label: 'Logs', component: LogsPage },
  { id: 'about', label: 'About', component: AboutPage },
];

export function pageComponent(id: PageId): ComponentType {
  return (PAGES.find((p) => p.id === id) ?? PAGES[0]).component;
}
