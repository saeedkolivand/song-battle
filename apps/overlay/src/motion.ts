import type { Transition, Variants } from 'framer-motion';

// Motion tokens for the overlay. Kept in one place so the feel is consistent and
// easy to tune. Callers pass `duration: 0` instead when reduced motion is on.
export const barSpring: Transition = { type: 'spring', stiffness: 120, damping: 22 };

export const matchSwap: Variants = {
  initial: { opacity: 0, y: 24, scale: 0.98 },
  animate: { opacity: 1, y: 0, scale: 1 },
  exit: { opacity: 0, y: -24, scale: 0.98 },
};

export const swapTransition: Transition = { duration: 0.4, ease: 'easeInOut' };
