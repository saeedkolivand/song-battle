import type { Transition, Variants } from 'framer-motion';

// Motion tokens for the overlay. Kept in one place so the feel is consistent and
// easy to tune. Callers pass `duration: 0` instead when reduced motion is on.
export const barSpring: Transition = { type: 'spring', stiffness: 120, damping: 22 };

// State crossfade (idle ↔ active ↔ winner) at the App level.
export const matchSwap: Variants = {
  initial: { opacity: 0, y: 22, scale: 0.98 },
  animate: { opacity: 1, y: 0, scale: 1 },
  exit: { opacity: 0, y: -22, scale: 0.98 },
};

export const swapTransition: Transition = { duration: 0.3, ease: 'easeInOut' };

// Staggered entrance for the two song cards + centre column on a new matchup.
export const cardsContainer: Variants = {
  initial: {},
  animate: { transition: { staggerChildren: 0.09, delayChildren: 0.04 } },
};

export const cardItem: Variants = {
  initial: { opacity: 0, y: 20, scale: 0.94 },
  animate: { opacity: 1, y: 0, scale: 1, transition: { duration: 0.34, ease: 'backOut' } },
};

// Reduced motion: opacity only, no positional jump.
export const cardItemReduced: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: 0.2 } },
};
