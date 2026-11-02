import { describe, it, expect } from 'vitest';
import i18n from '@/i18n/config';
import { receiptMessage, type IssueArchiveReceipt } from './issueArchiveNotice';
for (const language of ['en', 'ja']) {
  describe(`archive receipt presentation ${language}`, () => {
    const t = i18n.getFixedT(language, 'tasks');
    const receipt = (
      status: string,
      result: string | null
    ): IssueArchiveReceipt => ({ request_id: 'raw-request', status, result });
    it('accepts clean archive and restore success without a warning', () => {
      for (const result of [
        null,
        '{}',
        '{"workspaces":[]}',
        '{"issue_id":"raw-issue","archived":false}',
        '{"workspaces":[{"workspace_id":"raw-ws","deletion_error":null}]}',
      ])
        expect(receiptMessage(receipt('succeeded', result), t)).toBe('');
    });
    it.each(['accepted', 'running', 'uncertain', 'failed', 'future-status'])(
      'keeps %s unconfirmed, with safe request identity',
      (status) => {
        const text = receiptMessage(
          receipt(status, '{"message":"Raw secret /internal/path"}'),
          t
        );
        expect(text).toContain('raw-request');
        expect(text).toContain(
          language === 'ja'
            ? '自動で再実行しないでください'
            : 'do not repeat it automatically'
        );
        expect(text).not.toContain('Raw secret');
      }
    );
    it.each([
      '{',
      'null',
      '[]',
      'true',
      '{"workspaces":{}}',
      '{"workspaces":[null]}',
      '{"workspaces":[[]]}',
    ])('handles unreadable successful result %s without throwing', (result) => {
      expect(receiptMessage(receipt('succeeded', result), t)).toContain(
        language === 'ja'
          ? '詳細を読み取れませんでした'
          : 'details could not be read'
      );
    });
    it('retains workspace IDs while hiding arbitrary deletion errors and preserving the receipt', () => {
      const value = receipt(
        'succeeded',
        JSON.stringify({
          workspaces: [
            {
              workspace_id: 'Raw workspace 原文',
              deletion_error: 'Raw secret /host/path',
            },
            { workspace_id: 'clean', deletion_error: null },
            { deletion_error: 'other secret' },
          ],
        })
      );
      const before = JSON.stringify(value);
      const text = receiptMessage(value, t);
      expect(text).toContain('Raw workspace 原文');
      expect(text).toContain('raw-request');
      expect(text).toContain(
        language === 'ja' ? '不明なワークスペース' : 'Unknown workspace'
      );
      expect(text).not.toContain('secret');
      expect(JSON.stringify(value)).toBe(before);
    });
  });
}
