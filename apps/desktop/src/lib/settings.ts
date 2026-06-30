// Local UI preferences (accent colour, default timer). These are renderer-only
// conveniences persisted in localStorage — not battle state. The accent is wired
// to the Tailwind `--color-accent` token so every `accent` utility re-themes live.

const ACCENT_KEY = 'sb.accent';
const TIMER_KEY = 'sb.timerDefault';
const DEFAULT_ACCENT = '#34d399';
const DEFAULT_TIMER = 30;

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

export function getTimerDefault(): number {
  const raw = Number(localStorage.getItem(TIMER_KEY));
  return Number.isFinite(raw) && raw > 0 ? raw : DEFAULT_TIMER;
}

export function setTimerDefault(seconds: number): void {
  localStorage.setItem(TIMER_KEY, String(seconds));
}
