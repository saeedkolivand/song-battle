// jest-dom matchers (.toBeInTheDocument, .toHaveTextContent, …) wired into Vitest's
// expect, with the matching TS type augmentation for the 'vitest' module.
import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';

// usePersistentState / settings write to localStorage; clear it between tests so
// persisted UI state (e.g. the Kick provider mode) can't leak across cases.
afterEach(() => {
  localStorage.clear();
});
