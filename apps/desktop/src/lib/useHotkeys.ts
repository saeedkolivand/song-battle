import { useEffect } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { log } from '@sb/shared';
import { ipc } from './ipc';

export interface Hotkey {
  keys: string;
  label: string;
}

// In-app (NOT OS-global) hotkeys. Documented on the Settings page.
export const HOTKEYS: readonly Hotkey[] = [
  { keys: 'Space', label: 'Start match' },
  { keys: 'R', label: 'Reset votes' },
  { keys: 'S', label: 'Skip match' },
  { keys: 'O', label: 'Open overlay URL (browser)' },
  { keys: 'F', label: 'Open overlay window' },
];

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target.isContentEditable;
}

/**
 * Dashboard keyboard shortcuts. Ignored while typing in a field and when a
 * modifier is held, so they never hijack normal text entry or browser chrome.
 */
export function useGlobalHotkeys(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.repeat) return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (isTypingTarget(e.target)) return;

      const key = e.key.toLowerCase();
      let action: (() => Promise<unknown>) | null = null;
      if (e.code === 'Space' || key === ' ') action = () => ipc.startMatch();
      else if (key === 'r') action = () => ipc.resetVotes();
      else if (key === 's') action = () => ipc.skipMatch();
      else if (key === 'o') action = async () => openUrl(await ipc.overlayUrl());
      else if (key === 'f') action = () => ipc.openOverlayWindow();

      if (!action) return;
      e.preventDefault();
      action().catch((err: unknown) => log.error('hotkey failed', err));
    };

    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}
