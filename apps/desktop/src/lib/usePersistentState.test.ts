import { describe, it, expect } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { usePersistentState } from './usePersistentState';

describe('usePersistentState', () => {
  it('uses the initial value when nothing is stored, then persists updates', () => {
    const { result } = renderHook(() => usePersistentState('t.key', 'a'));
    expect(result.current[0]).toBe('a');

    act(() => result.current[1]('b'));
    expect(result.current[0]).toBe('b');
    expect(JSON.parse(localStorage.getItem('t.key')!)).toBe('b');
  });

  it('restores a previously stored value on mount', () => {
    localStorage.setItem('t.key2', JSON.stringify('stored'));
    const { result } = renderHook(() => usePersistentState('t.key2', 'default'));
    expect(result.current[0]).toBe('stored');
  });

  it('falls back to the initial value when the stored JSON is corrupt', () => {
    localStorage.setItem('t.key3', '{ not json');
    const { result } = renderHook(() => usePersistentState('t.key3', 'fallback'));
    expect(result.current[0]).toBe('fallback');
  });
});
