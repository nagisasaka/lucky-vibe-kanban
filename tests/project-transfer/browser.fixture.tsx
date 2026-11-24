import React, { useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ProjectIssueEditor } from '../../packages/web-core/src/shared/components/ProjectIssueEditor';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import {
  createRouter,
  createRootRoute,
  RouterProvider,
} from '@tanstack/react-router';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import english from '../../packages/web-core/src/i18n/locales/en/common.json';
import { ProjectTransferPanel } from '../../packages/web-core/src/shared/components/ProjectTransferPanel';
i18n.use(initReactI18next).init({
  lng: location.search.includes('english') ? 'en' : 'ja',
  resources: { ja: { common }, en: { common: english } },
  initImmediate: false,
});
const queryClient = new QueryClient();
function IssueText() {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(
    '[old link](https://old.invalid/private) ![attachment](/api/attachments/old-id) <img src="https://old.invalid/secret">'
  );
  return (
    <section id="imported-issue">
      <ProjectIssueEditor
        projectId="destination-project"
        disabled={!editing}
        value={value}
        onChange={setValue}
        onRequestEdit={() => setEditing(true)}
      />
    </section>
  );
}
const route = createRootRoute({
  component: () => (
    <main className="bg-primary text-normal p-base">
      <ProjectTransferPanel
        projectId={
          location.search.includes('import-only')
            ? undefined
            : 'destination-project'
        }
      />
      {!location.search.includes('import-only') && (
        <QueryClientProvider client={queryClient}>
          <IssueText />
        </QueryClientProvider>
      )}
    </main>
  ),
});
const router = createRouter({ routeTree: route });
createRoot(document.getElementById('root')!).render(
  <RouterProvider router={router} />
);
