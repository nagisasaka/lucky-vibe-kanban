import { it, expect } from 'vitest';
import i18n from '@/i18n/config';
import { receiptMessage } from './issueArchiveNotice';

it('keeps uncertain receipt guidance, request identity and original diagnostics across locale changes', () => {
  const receipt = {
    request_id: 'raw-request',
    status: 'uncertain',
    result: '{"message":"Original diagnostic"}',
  };
  for (const language of ['en', 'ja']) {
    const text = receiptMessage(receipt, i18n.getFixedT(language, 'tasks'));
    expect(text).toContain('raw-request');
    expect(text).toContain('Original diagnostic');
    expect(text).toContain(
      language === 'ja'
        ? '自動で再実行しないでください'
        : 'do not repeat it automatically'
    );
  }
  expect(receipt.status).toBe('uncertain');
});
