import { createRoot } from 'react-dom/client';
import { useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppChatMarkdown } from '@/features/workspace-chat/ui/AppChatMarkdown';
import { WorkspaceFilePreviewActionsProvider } from '@/features/workspace-files/model/WorkspaceFilePreviewActionsProvider';
import { WorkspaceFilePreviewPanel } from '@/features/workspace-files/ui/WorkspaceFilePreviewPanel';
import type { WorkspaceFilePreviewTarget } from '@/features/workspace-files/model/types';

// Only conversation text and page layout are fixtures. Production chat renderer,
// resolver, file viewer and all API calls are real; no route/transport mocks.
export function mount(workspaceId: string, root: string) {
  const content = [
    '[relative](src/example.ts:3)',
    '[absolute](' + root + '/project/src/example.ts#L4)',
    '[spaces](<src/space 日本語.txt:2>)',
    '[encoded](src/space%20%E6%97%A5%E6%9C%AC%E8%AA%9E.txt#L2)',
    '[missing](missing.txt)',
    '[symlink](escape.txt)',
    '[outside](/etc/passwd)',
    '[traversal](../secret.txt)',
    '[encoded-traversal](%2e%2e/secret.txt)',
    '[dangerous](javascript:alert)',
    '[external](https://example.com/docs)',
    '[markdown](README.md:2)',
  ].join('\n\n');
  function Fixture() {
    const [target, setTarget] = useState<WorkspaceFilePreviewTarget | null>(
      null
    );
    return (
      <QueryClientProvider client={client}>
        <WorkspaceFilePreviewActionsProvider
          onOpenWorkspaceFilePreview={setTarget}
        >
          <section aria-label="Chat">
            <AppChatMarkdown
              content={content}
              workspaceId={workspaceId}
              sessionId={undefined}
              className={undefined}
              maxWidth={undefined}
            />
          </section>
          <section aria-label="File preview" style={{ height: 300 }}>
            <WorkspaceFilePreviewPanel target={target} />
          </section>
        </WorkspaceFilePreviewActionsProvider>
      </QueryClientProvider>
    );
  }
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  document.body.innerHTML = '<div id="file-links-root"></div>';
  createRoot(document.getElementById('file-links-root')!).render(<Fixture />);
}
