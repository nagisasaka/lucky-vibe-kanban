import React, { lazy, Suspense } from 'react';
import ReactDOM from 'react-dom/client';
import * as Sentry from '@sentry/react';
import { ClickToComponent } from 'click-to-react-component';
import { QueryClientProvider } from '@tanstack/react-query';
import posthog from 'posthog-js';
import { PostHogProvider } from 'posthog-js/react';
import { CrashScreen } from '@vibe/ui/components/CrashScreen';
import i18n from '@/i18n';
import '@/app/styles/new/index.css';
import { ServerAccessGate } from '@web/app/auth/ServerAccessGate';
import { installAccessFetch } from '@web/app/auth/serverAccess';
import '@/shared/types/modals';
import { queryClient } from '@/shared/lib/queryClient';

import { DevAgentation } from './DevAgentation';

const App = lazy(() => import('@web/app/entry/App'));

if (
  import.meta.env.VITE_POSTHOG_API_KEY &&
  import.meta.env.VITE_POSTHOG_API_ENDPOINT
) {
  posthog.init(import.meta.env.VITE_POSTHOG_API_KEY, {
    api_host: import.meta.env.VITE_POSTHOG_API_ENDPOINT,
    capture_pageview: false,
    capture_pageleave: true,
    capture_performance: true,
    autocapture: false,
    opt_out_capturing_by_default: true,
  });
} else {
  console.warn(
    'PostHog API key or endpoint not set. Analytics will be disabled.'
  );
}

installAccessFetch();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <PostHogProvider client={posthog}>
        <Sentry.ErrorBoundary
          fallback={({ error, componentStack }) => (
            <CrashScreen
              error={error instanceof Error ? error : undefined}
              componentStack={componentStack}
            />
          )}
          showDialog
        >
          <ClickToComponent />
          <ServerAccessGate>
            <Suspense
              fallback={
                <div className="flex h-screen items-center justify-center bg-primary text-low">
                  {i18n.t('common:states.loading')}
                </div>
              }
            >
              <App />
              <DevAgentation />
            </Suspense>
          </ServerAccessGate>
        </Sentry.ErrorBoundary>
      </PostHogProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
