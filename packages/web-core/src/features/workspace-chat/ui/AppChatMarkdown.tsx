import { ChatMarkdown } from '@vibe/ui/components/ChatMarkdown';
import WYSIWYGEditor from '@/shared/components/WYSIWYGEditor';
import { useChangesViewActions } from '@/shared/hooks/useChangesView';
import { useChatFileLink } from '@/features/workspace-files/model/useChatFileLink';

export function AppChatMarkdown({
  content,
  workspaceId,
  sessionId,
  className,
  maxWidth,
}: {
  content: string;
  workspaceId: string | undefined;
  sessionId: string | undefined;
  className: string | undefined;
  maxWidth: string | undefined;
}) {
  const { viewFileInChanges, findMatchingDiffPath } = useChangesViewActions();
  const resolveLocalLink = useChatFileLink(workspaceId, sessionId);

  return (
    <ChatMarkdown
      content={content}
      workspaceId={workspaceId}
      className={className}
      maxWidth={maxWidth}
      renderContent={({ content, className, workspaceId }) => (
        <WYSIWYGEditor
          value={content}
          resolveLocalLink={resolveLocalLink}
          disabled
          className={className}
          workspaceId={workspaceId}
          sessionId={sessionId}
          findMatchingDiffPath={findMatchingDiffPath}
          onCodeClick={viewFileInChanges}
        />
      )}
    />
  );
}
