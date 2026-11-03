import { describe, expect, it, vi } from 'vitest';
import {
  persistPetPreferences,
  registerPetPreferencesWriter,
} from './petPreferencesPersistence';

describe('acknowledged pet preferences', () => {
  it('fails closed before the scratch snapshot or after disconnect', async () => {
    await expect(persistPetPreferences(null)).rejects.toThrow('not connected');
    const unregister = registerPetPreferencesWriter(async () => undefined);
    unregister();
    await expect(persistPetPreferences(null)).rejects.toThrow('not connected');
  });
  it('waits for the existing writer and propagates a failed save', async () => {
    let resolve!: () => void;
    const acknowledged = vi.fn();
    const unregister = registerPetPreferencesWriter(
      () =>
        new Promise<void>((r) => {
          resolve = r;
        })
    );
    const pending = persistPetPreferences({ hidden: true }).then(acknowledged);
    await Promise.resolve();
    expect(acknowledged).not.toHaveBeenCalled();
    resolve();
    await pending;
    expect(acknowledged).toHaveBeenCalledOnce();
    unregister();
    const remove = registerPetPreferencesWriter(async () => {
      throw new Error('save failed');
    });
    await expect(persistPetPreferences(null)).rejects.toThrow('save failed');
    remove();
  });
  it('stale cleanup cannot unregister a new connection writer', async () => {
    const stale = registerPetPreferencesWriter(async () => {
      throw new Error('old');
    });
    const current = vi.fn(async () => undefined);
    const unregister = registerPetPreferencesWriter(current);
    stale();
    await persistPetPreferences({ hidden: false });
    expect(current).toHaveBeenCalledWith({ hidden: false });
    unregister();
  });
});
