import type { TextareaHTMLAttributes } from 'react';

const cls =
  'w-full rounded-xl border border-white/15 bg-black/30 px-3 py-2 text-sm text-white ' +
  'placeholder:text-white/30 transition-colors focus-visible:outline-none ' +
  'focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40';

export function TextArea({ className = '', rows = 3, ...rest }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea rows={rows} className={`${cls} ${className}`} {...rest} />;
}
