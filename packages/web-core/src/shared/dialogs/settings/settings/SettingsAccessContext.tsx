import { createContext, useContext, type ReactNode } from 'react';

/** The local application supplies its authenticated management screen.
 * Shared settings never fetch or infer server management privileges. */
export const SettingsAccessContext = createContext<ReactNode>(null);
export const useSettingsAccess = () => useContext(SettingsAccessContext);
