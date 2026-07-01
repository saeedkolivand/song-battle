import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';

// A drop-in `useState` that survives navigation and reload by mirroring to
// localStorage — the same convention as the accent/OBS settings. Use ONLY for
// renderer-only UI/form state: backend-owned data (battles, votes, settings,
// Kick creds) lives in the Rust DB, and secrets must never be stored here.
export function usePersistentState<T>(key: string, initial: T): [T, Dispatch<SetStateAction<T>>] {
  const [value, setValue] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(key);
      return raw === null ? initial : (JSON.parse(raw) as T);
    } catch {
      return initial; // disabled/corrupt storage — fall back to the default
    }
  });

  useEffect(() => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      // ignore quota errors / storage disabled — persistence is best-effort
    }
  }, [key, value]);

  return [value, setValue];
}
