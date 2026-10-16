import { ReactNode, useCallback, useEffect, useMemo } from 'react';
import type { Config, LoginStatus } from 'shared/types';
import { configApi } from '@/shared/lib/api';
import { updateLanguageFromConfig } from '@/i18n/config';
import { useUserSystemController } from '@/shared/hooks/useUserSystemController';
import { UserSystemContext } from '@/shared/hooks/useUserSystem';
import { currentLocalUserId } from '@/shared/lib/localIdentity';

interface UserSystemProviderProps {
  children: ReactNode;
}

export function UserSystemProvider({ children }: UserSystemProviderProps) {
  const loadConfig = useCallback(() => configApi.getConfig(null), []);
  const saveConfig = useCallback(
    (config: Parameters<typeof configApi.saveConfig>[0]) =>
      configApi.saveConfig(config, null),
    []
  );

  const { value } = useUserSystemController({
    queryKey: ['user-system', 'local'],
    load: loadConfig,
    save: saveConfig,
  });
  const { updateConfig, updateAndSaveConfig } = value;

  const localMode = true;
  const localLoginStatus = useMemo<LoginStatus>(
    () => ({
      status: 'loggedin',
      profile: {
        user_id: currentLocalUserId(),
        username: 'local',
        email: 'local@vibe-kanban.local',
        providers: [
          {
            provider: 'local',
            username: 'local',
            display_name: 'Local User',
            email: 'local@vibe-kanban.local',
            avatar_url: null,
          },
        ],
      },
    }),
    []
  );
  const localConfig = useMemo<Config | null>(
    () =>
      localMode && value.config
        ? {
            ...value.config,
            onboarding_acknowledged: true,
            disclaimer_acknowledged: true,
          }
        : value.config,
    [localMode, value.config]
  );
  const localUpdateConfig = useCallback(
    (updates: Partial<Config>) =>
      updateConfig(
        localMode
          ? {
              ...updates,
              onboarding_acknowledged: true,
              disclaimer_acknowledged: true,
            }
          : updates
      ),
    [localMode, updateConfig]
  );
  const localUpdateAndSaveConfig = useCallback(
    (updates: Partial<Config>) =>
      updateAndSaveConfig(
        localMode
          ? {
              ...updates,
              onboarding_acknowledged: true,
              disclaimer_acknowledged: true,
            }
          : updates
      ),
    [localMode, updateAndSaveConfig]
  );
  const contextValue = useMemo(
    () =>
      localMode
        ? {
            ...value,
            config: localConfig,
            updateConfig: localUpdateConfig,
            updateAndSaveConfig: localUpdateAndSaveConfig,
            system: {
              ...value.system,
              config: localConfig,
              loginStatus: localLoginStatus,
            },
            loginStatus: localLoginStatus,
          }
        : value,
    [
      localConfig,
      localLoginStatus,
      localMode,
      localUpdateAndSaveConfig,
      localUpdateConfig,
      value,
    ]
  );

  // Sync language with i18n when config changes
  useEffect(() => {
    if (value.config?.language) {
      updateLanguageFromConfig(value.config.language);
    }
  }, [value.config?.language]);

  return (
    <UserSystemContext.Provider value={contextValue}>
      {children}
    </UserSystemContext.Provider>
  );
}
