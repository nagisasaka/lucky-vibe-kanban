import { useQuery } from '@tanstack/react-query';
import {
  projectTransferApi,
  type TransferReport,
} from '@/shared/lib/projectTransferApi';

/** Only a confirmed non-imported Project may resolve authored Markdown links. */
export function mustKeepProjectTextInert(
  status: string,
  report: TransferReport | null | undefined
): boolean {
  return status !== 'success' || report !== null;
}
export function useImportedProject(projectId: string) {
  const query = useQuery({
    queryKey: ['project-transfer-report', projectId],
    queryFn: () => projectTransferApi.report(projectId),
    enabled: Boolean(projectId),
    staleTime: 30000,
    retry: false,
  });
  return {
    ...query,
    keepTextInert: mustKeepProjectTextInert(query.status, query.data),
  };
}
