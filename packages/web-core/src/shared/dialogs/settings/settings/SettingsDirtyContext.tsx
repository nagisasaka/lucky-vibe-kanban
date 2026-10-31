import { useContext, useState, useCallback, useRef } from 'react';
import { createHmrContext } from '@/shared/lib/hmrContext';
import type { ReactNode } from 'react';

interface SettingsDirtyContextValue {
  isDirty: boolean;
  getIsDirty: () => boolean;
  setDirty: (sectionId: string, dirty: boolean) => void;
  clearAll: () => void;
}

const SettingsDirtyContext = createHmrContext<SettingsDirtyContextValue | null>(
  'SettingsDirtyContext',
  null
);

export function SettingsDirtyProvider({ children }: { children: ReactNode }) {
  const [dirtySections, setDirtySections] = useState<Set<string>>(new Set());

  const currentSections = useRef(dirtySections);
  const setDirty = useCallback((sectionId: string, dirty: boolean) => {
    const next = new Set(currentSections.current);
    if (dirty) next.add(sectionId);
    else next.delete(sectionId);
    currentSections.current = next;
    setDirtySections(next);
  }, []);
  const getIsDirty = useCallback(() => currentSections.current.size > 0, []);
  const clearAll = useCallback(() => {
    currentSections.current = new Set();
    setDirtySections(currentSections.current);
  }, []);

  const isDirty = dirtySections.size > 0;

  return (
    <SettingsDirtyContext.Provider
      value={{ isDirty, getIsDirty, setDirty, clearAll }}
    >
      {children}
    </SettingsDirtyContext.Provider>
  );
}

export function useSettingsDirty() {
  const context = useContext(SettingsDirtyContext);
  if (!context) {
    throw new Error(
      'useSettingsDirty must be used within a SettingsDirtyProvider'
    );
  }
  return context;
}
