import * as Sentry from '@sentry/react';
import { RouterProvider } from '@tanstack/react-router';
import { HotkeysProvider } from 'react-hotkeys-hook';
import { UserSystemProvider } from '@web/app/providers/ConfigProvider';
import { localAppNavigation } from '@web/app/navigation/AppNavigation';
import { LocalAuthProvider } from '@/shared/providers/auth/LocalAuthProvider';
import { AppRuntimeProvider } from '@/shared/hooks/useAppRuntime';
import { AppNavigationProvider } from '@/shared/hooks/useAppNavigation';
import { router } from '@web/app/router';

if (import.meta.env.VITE_SENTRY_DSN) {
  Sentry.init({
    dsn: import.meta.env.VITE_SENTRY_DSN,
    tracesSampleRate: 1.0,
    environment: import.meta.env.MODE === 'development' ? 'dev' : 'production',
    integrations: [Sentry.tanstackRouterBrowserTracingIntegration(router)],
  });
  Sentry.setTag('source', 'frontend');
}

function App() {
  return (
    <AppRuntimeProvider runtime="local">
      <AppNavigationProvider value={localAppNavigation}>
        <UserSystemProvider>
          <LocalAuthProvider>
            <HotkeysProvider
              initiallyActiveScopes={[
                'global',
                'workspace',
                'kanban',
                'projects',
              ]}
            >
              <RouterProvider router={router} />
            </HotkeysProvider>
          </LocalAuthProvider>
        </UserSystemProvider>
      </AppNavigationProvider>
    </AppRuntimeProvider>
  );
}

export default App;
