import type { ReactNode } from 'react';

// Glassmorphism card shared by dashboard and overlay. Tailwind v4 utility classes;
// the consumer app provides the Tailwind build.
export function Card({ children, className = '' }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={`rounded-2xl border border-white/10 bg-white/5 p-6 shadow-xl backdrop-blur-md ${className}`}
    >
      {children}
    </div>
  );
}
