import { describe, expect, it } from 'vitest';
import type { IntegrationRun } from 'shared/types';
import {
  isIntegrationInitialMessage,
  savedIntegrationInput,
} from './integrationInitialMessage';

function run(content: string): IntegrationRun {
  return {
    payload: {
      instructions: 'original additional instructions',
      worker_dispatch: {
        type: 'create',
        data: {
          request: {
            agent_run_id: 'initial-run',
            input: { message_id: 'initial-message', content },
          },
        },
      },
    },
  } as unknown as IntegrationRun;
}

describe('saved Auto-merge initial input', () => {
  it.each(['old Skill/Issue/Manifest full input', 'short fixed contract'])(
    'identifies only the saved message: %s',
    (content) => {
      const record = run(content);
      expect(
        isIntegrationInitialMessage(
          record,
          'initial-run',
          'initial-message',
          content
        )
      ).toBe(true);
      expect(
        isIntegrationInitialMessage(
          record,
          'followup-run',
          'initial-message',
          content
        )
      ).toBe(false);
      expect(
        isIntegrationInitialMessage(
          record,
          'initial-run',
          'followup-message',
          content
        )
      ).toBe(false);
      expect(
        isIntegrationInitialMessage(
          record,
          'initial-run',
          'initial-message',
          'current regenerated input'
        )
      ).toBe(false);
      expect(savedIntegrationInput(record)?.input.content).toBe(content);
    }
  );
  it('does not infer provenance from message text or unavailable dispatch', () => {
    const record = run('自動マージ');
    record.payload.worker_dispatch = null;
    expect(
      isIntegrationInitialMessage(
        record,
        'initial-run',
        'initial-message',
        '自動マージ'
      )
    ).toBe(false);
    expect(savedIntegrationInput(record)).toBeNull();
  });
});
