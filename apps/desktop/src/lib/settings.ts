// Local UI preference: accent colour only. Pure renderer state persisted in
// localStorage and wired to the Tailwind `--color-accent` token so every `accent`
// utility re-themes live. The default-timer setting moved to the backend
// (get_settings / set_default_timer) in Phase 2.

const ACCENT_KEY = 'sb.accent';
const DEFAULT_ACCENT = '#34d399';

export interface Accent {
  name: string;
  value: string;
}

export const ACCENTS: readonly Accent[] = [
  { name: 'Emerald', value: '#34d399' },
  { name: 'Violet', value: '#a78bfa' },
  { name: 'Sky', value: '#38bdf8' },
  { name: 'Rose', value: '#fb7185' },
  { name: 'Amber', value: '#fbbf24' },
];

export function getAccent(): string {
  return localStorage.getItem(ACCENT_KEY) ?? DEFAULT_ACCENT;
}

export function applyAccent(): void {
  document.documentElement.style.setProperty('--color-accent', getAccent());
}

export function setAccent(value: string): void {
  localStorage.setItem(ACCENT_KEY, value);
  applyAccent();
}
