import { createContext, useContext, type ReactNode } from 'react';
import { workspacesApi } from '@/shared/lib/api';
const Context = createContext(workspacesApi.files);
export function FileApiProvider({
  api,
  children,
}: {
  api: typeof workspacesApi.files;
  children: ReactNode;
}) {
  return <Context.Provider value={api}>{children}</Context.Provider>;
}
export const useFileApi = () => useContext(Context);
