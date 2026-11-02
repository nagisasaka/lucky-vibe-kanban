// Pure production-component fixture: no backend, real approval or provider.
import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { ExecutionApprovalDetails } from '../../packages/ui/src/components/ExecutionApprovalDetails';
import { ChatToolSummary } from '../../packages/ui/src/components/ChatToolSummary';
import {
  SessionChatBox,
  type SessionRuntimeActionPolicy,
} from '../../packages/ui/src/components/SessionChatBox';
import { TooltipProvider } from '../../packages/ui/src/components/RadixTooltip';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import tasks from '../../packages/web-core/src/i18n/locales/ja/tasks.json';
void i18n.use(initReactI18next).init({
  lng: 'ja',
  interpolation: { escapeValue: false },
  resources: { ja: { common, tasks } },
  initImmediate: false,
});
function Fixture() {
  const [thirdParty, setThirdParty] = useState(false);
  const [empty, setEmpty] = useState(false);
  const [timedOut, setTimedOut] = useState(false);
  const [value, onChange] = useState('');
  const [result, setResult] = useState('No mock decision');
  const server = thirdParty ? 'third_party_example' : 'lvk_fixture';
  const tool = thirdParty ? 'run_process' : 'get_dev_server_status';
  const label = `MCP third-party server=${JSON.stringify(server)} tool=${JSON.stringify(tool)} (one call only)`;
  const args = {
    project_id: '00000000-0000-0000-0000-000000000001',
    workspace_id: '00000000-0000-0000-0000-000000000002',
    authorization: '[REDACTED: synthetic placeholder]',
  };
  const decision = (text: string) => () => setResult(`MOCK ONLY: ${text}`);
  // Explicit mock policy, not a real permission grant or API decision.
  const actionPolicy = Object.fromEntries(
    [
      'send_initial',
      'send_follow_up',
      'queue_follow_up',
      'cancel_queue',
      'stop',
      'approve',
      'request_changes',
      'answer_question',
      'retry',
      'resume',
    ].map((action) => {
      const allowed = action === 'stop' || (action === 'approve' && !timedOut);
      return [
        action,
        {
          action,
          allowed,
          reason: allowed
            ? null
            : action === 'approve'
              ? 'approval_timed_out'
              : 'no_content',
        },
      ];
    })
  ) as SessionRuntimeActionPolicy;
  return (
    <TooltipProvider>
      <main className="p-double bg-primary text-normal space-y-base">
        <p>
          LOCAL-206 component mock — no real permission response, API or
          provider
        </p>
        <button
          className="border text-normal p-half"
          onClick={() => setThirdParty(!thirdParty)}
        >
          Toggle third-party tool
        </button>
        <button
          className="border text-normal p-half"
          onClick={() => setTimedOut(!timedOut)}
        >
          Toggle timeout
        </button>
        <button onClick={() => setEmpty(!empty)}>Toggle empty arguments</button>
        <ChatToolSummary
          summary={label}
          toolName={`${server}/${tool}`}
          status={{
            status: 'pending_approval',
            approval_id: 'mock-service-owned-id',
          }}
          active={true}
        />
        <pre id="mcp-target" className="whitespace-pre-wrap break-words">
          {JSON.stringify(
            { server, tool, scope: 'one call only', arguments: args },
            null,
            2
          )}
        </pre>
        <output>{result}</output>
        <SessionChatBox
          isMobile={window.innerWidth < 640}
          status="running"
          actionPolicy={actionPolicy}
          editor={{ value, onChange }}
          renderEditor={({ value, onChange, disabled }) => (
            <textarea
              aria-label="mock feedback"
              value={value}
              disabled={disabled}
              onChange={(e) => onChange(e.target.value)}
            />
          )}
          actions={{
            onSend: decision('send'),
            onQueue: decision('queue'),
            onStop: decision('stop'),
            onCancelQueue: decision('cancel queue'),
            onPasteFiles: () => {},
          }}
          session={{ sessions: [], onSelectSession: () => {} }}
          approvalMode={{
            isActive: true,
            details: (
              <ExecutionApprovalDetails
                projectId="project-fixture"
                workspaceId="workspace-fixture"
                sessionId="session-fixture"
                agentRunId="run-fixture"
                approvalId="mock-service-owned-id"
                toolName={label}
                operation={`${server}/${tool}`}
                arguments={empty ? {} : args}
              />
            ),
            isSubmitting: false,
            isTimedOut: timedOut,
            onApprove: decision('approve'),
            onDeny: decision('deny'),
            onRequestChanges: decision('request changes'),
          }}
        />
      </main>
    </TooltipProvider>
  );
}
createRoot(document.getElementById('root')!).render(<Fixture />);
