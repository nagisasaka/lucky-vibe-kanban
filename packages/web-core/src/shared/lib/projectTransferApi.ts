import { makeLocalApiRequest } from './localApiTransport';
import { handleApiResponse } from './api';

export interface TransferSummary {
  coverage: string;
  missing_attachments: { name: string; size_bytes: number | null }[];
  external_reference_count: number;
  history_included?: boolean;
  history_count?: number;
  manifest_count?: number;
  history_omissions?: string;
}
export interface TransferPreview extends TransferSummary {
  sha256: string;
  project_name: string;
  issue_count: number;
  version_count: number;
  comment_count: number;
  repositories: { source_id: string; display_name: string }[];
}
export interface TransferReport extends TransferSummary {
  project_id: string;
}
export interface ImportedHistory {
  issue_id: string;
  workspace_id: string;
  session_id: string;
  run_id: string;
  status: string;
  messages: { role: string; content: string }[];
  manifests: {
    id: string;
    repo_id: string;
    created_at?: string;
    base_commit?: string;
    source_commit?: string;
    target_branch?: string | null;
    changed_paths?: string[];
    semantics: Record<string, unknown>;
  }[];
}
export function transferBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}
export const projectTransferApi = {
  export: async (projectId: string, includeHistory: boolean) => {
    const response = await makeLocalApiRequest(
      `/api/projects/${projectId}/transfer/export`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ include_history: includeHistory }),
      }
    );
    if (!response.ok) {
      await handleApiResponse(response);
      throw new Error('Export failed');
    }
    return response.blob();
  },
  preview: async (bytes: Uint8Array) =>
    handleApiResponse<TransferPreview>(
      await makeLocalApiRequest('/api/project-transfer/preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/octet-stream' },
        body: bytes as BodyInit,
      })
    ),
  import: async (
    bytes: Uint8Array,
    expectedSha256: string,
    key: string,
    repoMap: Record<string, string>
  ) =>
    handleApiResponse<TransferReport>(
      await makeLocalApiRequest('/api/project-transfer/import', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          data_base64: transferBase64(bytes),
          expected_sha256: expectedSha256,
          key,
          repo_map: repoMap,
        }),
      })
    ),
  report: async (projectId: string) =>
    handleApiResponse<TransferReport | null>(
      await makeLocalApiRequest(`/api/projects/${projectId}/transfer/report`)
    ),
  history: async (projectId: string) =>
    handleApiResponse<ImportedHistory[]>(
      await makeLocalApiRequest(`/api/projects/${projectId}/transfer/history`)
    ),
};
