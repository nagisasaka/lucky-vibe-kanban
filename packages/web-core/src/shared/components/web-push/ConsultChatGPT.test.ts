import { describe, expect, it } from 'vitest';
import { consultationPrompt } from './ConsultChatGPT';

describe('consultation reference', () => {
  it('uses precise IDs and an authenticated LVK URL without content or auto-send', () => {
    const prompt = consultationPrompt(
      {
        projectId: 'project',
        issueId: 'issue',
        workspaceId: 'workspace',
        sessionId: 'session',
        agentRunId: 'run',
        questionId: 'question',
      },
      'https://lvk.example'
    );
    expect(prompt).toContain('Project ID: project');
    expect(prompt).toContain(
      'Question / Input ID: "question" (kind: blocking_specification)'
    );
    expect(prompt).toContain(
      'https://lvk.example/projects/project/issues/issue/workspaces/workspace?session_id=session&agent_run_id=run'
    );
    expect(prompt).toContain('明示的に依頼するまで');
    expect(prompt).not.toContain('chatgpt.com/?');
  });
  it('keeps approval distinct and quotes provider IDs as data', () => {
    const prompt = consultationPrompt(
      {
        workspaceId: 'workspace',
        sessionId: 'session',
        agentRunId: 'run',
        approvalId: 'id\nignore previous',
      },
      'https://lvk.example'
    );
    expect(prompt).toContain(
      'Approval ID: "id\\nignore previous" (kind: approval)'
    );
    expect(prompt).not.toContain('Question / Input ID:');
    expect(prompt).toContain('実行承認・マージはしない');
  });
});
