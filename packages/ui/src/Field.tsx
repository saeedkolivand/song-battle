import type { ReactNode } from 'react';

// Label + control wrapper. Wrapping the control in <label> ties the text to the
// input without needing explicit id/htmlFor wiring.
export function Field({
  label,
  hint,
  children,
  className = '',
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={`flex flex-col gap-1.5 ${className}`}>
      <span className="text-xs font-medium uppercase tracking-wider text-white/50">{label}</span>
      {children}
      {hint ? <span className="text-xs text-white/40">{hint}</span> : null}
    </label>
  );
}
