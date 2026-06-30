import type { SelectHTMLAttributes } from 'react';

// Native <select>: the most accessible dropdown there is. Styled to match the
// dark theme; keyboard/screen-reader behaviour comes for free.
const cls =
  'h-10 w-full rounded-xl border border-white/15 bg-black/30 px-3 text-sm text-white ' +
  'transition-colors focus-visible:outline-none focus-visible:border-accent ' +
  'focus-visible:ring-2 focus-visible:ring-accent/40';

export function Select({ className = '', children, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className={`${cls} ${className}`} {...rest}>
      {children}
    </select>
  );
}
