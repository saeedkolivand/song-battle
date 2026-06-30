import type { ButtonHTMLAttributes } from 'react';

// Shared button primitive. Focus-visible ring (WCAG 2.4.7), ≥32px tall target
// (2.5.8), disabled affordance. `accent`/`ring-accent` come from the consuming
// app's Tailwind `@theme` token, so a runtime accent override re-themes buttons.
type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
type Size = 'sm' | 'md';

const base =
  'inline-flex select-none items-center justify-center gap-2 rounded-xl font-medium ' +
  'transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ' +
  'focus-visible:ring-offset-2 focus-visible:ring-offset-black ' +
  'disabled:cursor-not-allowed disabled:opacity-40';

const variants: Record<Variant, string> = {
  primary: 'bg-accent text-black hover:brightness-110',
  secondary: 'border border-white/15 bg-white/5 text-white hover:bg-white/10',
  ghost: 'text-white/70 hover:bg-white/10 hover:text-white',
  danger: 'border border-red-500/30 bg-red-500/10 text-red-200 hover:bg-red-500/20',
};

const sizes: Record<Size, string> = {
  sm: 'h-8 px-3 text-sm',
  md: 'h-10 px-4 text-sm',
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
}

export function Button({
  variant = 'secondary',
  size = 'md',
  className = '',
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    // eslint-disable-next-line react/button-has-type -- type defaulted/forwarded above
    <button type={type} className={`${base} ${variants[variant]} ${sizes[size]} ${className}`} {...rest} />
  );
}
