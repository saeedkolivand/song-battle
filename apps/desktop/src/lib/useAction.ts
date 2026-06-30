import { useCallback, useState } from 'react';

// One place for the await→catch discipline: never flip UI to success before the
// command resolves; surface the rejection as a message instead of swallowing it.
export interface ActionState {
  pending: boolean;
  error: string | null;
  run: (fn: () => Promise<unknown>) => Promise<boolean>;
  clearError: () => void;
}

export function useAction(): ActionState {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (fn: () => Promise<unknown>): Promise<boolean> => {
    setPending(true);
    setError(null);
    try {
      await fn();
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    } finally {
      setPending(false);
    }
  }, []);

  const clearError = useCallback(() => setError(null), []);
  return { pending, error, run, clearError };
}
