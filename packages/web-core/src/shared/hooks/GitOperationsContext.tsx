import { useTranslation } from 'react-i18next';
import React, { useCallback, useContext, useEffect, useState } from 'react';
import { createHmrContext } from '@/shared/lib/hmrContext';

type GitOperationNotice = 'rebase' | 'forcePush' | 'push' | 'targetBranch';

type GitOperationsContextType = {
  error: string | null;
  setError: (error: GitOperationNotice | null, detail?: string) => void;
};

const GitOperationsContext = createHmrContext<GitOperationsContextType | null>(
  'GitOperationsContext',
  null
);

export const GitOperationsProvider: React.FC<{
  workspaceId: string | undefined;
  children: React.ReactNode;
}> = ({ workspaceId, children }) => {
  const { t } = useTranslation('tasks');
  const [notice, setNotice] = useState<GitOperationNotice | null>(null);
  const [detail, setDetail] = useState<string | undefined>();
  const setError = useCallback(
    (notice: GitOperationNotice | null, detail?: string) => {
      setNotice(notice);
      setDetail(detail);
    },
    []
  );
  const messages: Record<GitOperationNotice, string> = {
    rebase: t('gitNotice.rebaseUnconfirmed'),
    forcePush: t('gitNotice.forcePushUnconfirmed'),
    push: t('gitNotice.pushUnconfirmed'),
    targetBranch: t('gitNotice.targetUnconfirmed'),
  };
  const error = notice
    ? `${messages[notice]}${detail ? ` — ${detail}` : ''}`
    : null;

  useEffect(() => {
    setError(null);
  }, [workspaceId]);

  return (
    <GitOperationsContext.Provider value={{ error, setError }}>
      {children}
    </GitOperationsContext.Provider>
  );
};

export const useGitOperationsError = () => {
  const ctx = useContext(GitOperationsContext);
  if (!ctx) {
    throw new Error(
      'useGitOperationsError must be used within GitOperationsProvider'
    );
  }
  return ctx;
};
