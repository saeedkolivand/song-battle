import type { InputHTMLAttributes } from 'react';

const cls =
  'h-10 w-full rounded-xl border border-white/15 bg-black/30 px-3 text-sm text-white ' +
  'placeholder:text-white/30 transition-colors focus-visible:outline-none ' +
  'focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40';

export function Input({ className = '', ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={`${cls} ${className}`} {...rest} />;
}
