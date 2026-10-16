import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from 'react';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import {
  createMachineClient,
  type MachineClient,
  type MachineTarget,
} from '@/shared/lib/machineClient';
export type SettingsHostTargetId = string;
export type SettingsHostTarget = MachineTarget & {
  description?: string;
  status?: 'online' | 'offline';
};
const localHost: SettingsHostTarget = {
  id: 'local',
  kind: 'local',
  apiHostId: null,
  label: 'LVK server',
};
const SettingsHostContext = createContext({
  availableHosts: [localHost],
  hostsResolved: true,
  selectedHostId: 'local',
  selectedHost: localHost,
  canEdit: true,
  discoveryFailed: false,
  setSelectedHostId: (id: string) => {
    if (id !== 'local') throw new Error('Remote hosts are no longer supported');
  },
});
export function SettingsHostProvider({
  initialHostId,
  children,
}: {
  initialHostId?: string;
  children: ReactNode;
}) {
  if (initialHostId && initialHostId !== 'local') {
    throw new Error('Remote Access hosts are no longer supported');
  }
  return <>{children}</>;
}
export function useSettingsHost() {
  const context = useContext(SettingsHostContext);
  if (!context) {
    throw new Error(
      'useSettingsHost must be used within a SettingsHostProvider'
    );
  }
  return context;
}

export function useSettingsMachineClient(): MachineClient | null {
  const runtime = useAppRuntime();
  const { selectedHost, canEdit } = useSettingsHost();
  const current = useRef({ selectedHost, canEdit });
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  current.current = { selectedHost, canEdit };
  const targetId = selectedHost?.id;

  return useMemo(() => {
    const host = current.current.selectedHost;
    if (!host) {
      return null;
    }

    return createMachineClient(
      runtime,
      host,
      () =>
        mounted.current &&
        current.current.canEdit &&
        current.current.selectedHost?.id === targetId
    );
    // Machine identity is stable across status refreshes. The live guard above
    // still rejects writes from stale callbacks after a switch or disconnect.
  }, [runtime, targetId]);
}
