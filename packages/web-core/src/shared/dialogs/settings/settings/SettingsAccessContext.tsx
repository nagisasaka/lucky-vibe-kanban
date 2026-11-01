import { createContext, useContext, type ReactNode } from 'react';

/** The local app supplies account UI and its authorised management screen.
 * Shared settings never fetch or infer server management privileges. */
export const SettingsAccessContext = createContext<{
  account: ReactNode;
  management: ReactNode;
} | null>(null);
export const useSettingsAccess = () => useContext(SettingsAccessContext);
