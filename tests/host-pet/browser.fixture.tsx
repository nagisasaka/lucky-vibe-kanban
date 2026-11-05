import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '../../packages/web-core/src/i18n/config';
import { AppRuntimeProvider } from '../../packages/web-core/src/shared/hooks/useAppRuntime';
import { SettingsDirtyProvider } from '../../packages/web-core/src/shared/dialogs/settings/settings/SettingsDirtyContext';
import { HostPetSettingsSection } from '../../packages/web-core/src/shared/dialogs/settings/settings/HostPetSettingsSection';
import { EnvironmentPanel } from '../../packages/web-core/src/shared/components/EnvironmentPanel';
import { registerPetPreferencesWriter } from '../../packages/web-core/src/shared/lib/petPreferencesPersistence';
import { useUiPreferencesStore } from '../../packages/web-core/src/shared/stores/useUiPreferencesStore';
void i18n.changeLanguage(
  new URLSearchParams(location.search).get('lang') || 'ja'
);
// Only ephemeral fixture preferences; no user browser or shared data is touched.
useUiPreferencesStore.setState({
  petPreferences: JSON.parse(localStorage.getItem('fixture-pet') || 'null'),
});
registerPetPreferencesWriter(async (value) => {
  localStorage.setItem('fixture-pet', JSON.stringify(value));
});
createRoot(document.getElementById('root')!).render(
  <AppRuntimeProvider runtime="local">
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <SettingsDirtyProvider>
        <main className="min-h-screen bg-primary text-normal grid min-w-0 gap-6 lg:grid-cols-[300px_minmax(0,1fr)] p-2">
          <aside className="w-[300px] max-w-full min-w-0">
            <EnvironmentPanel />
          </aside>
          <section className="min-w-0 max-w-[800px] p-4">
            <HostPetSettingsSection />
          </section>
        </main>
      </SettingsDirtyProvider>
    </QueryClientProvider>
  </AppRuntimeProvider>
);
