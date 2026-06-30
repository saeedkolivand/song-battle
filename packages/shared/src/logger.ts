// ponytail: console wrapper with a tag. Swap for a real sink (dev panel / file) in Phase 7.
const tag = '[ksb]';

export const log = {
  info: (...a: unknown[]) => console.info(tag, ...a),
  warn: (...a: unknown[]) => console.warn(tag, ...a),
  error: (...a: unknown[]) => console.error(tag, ...a),
};
