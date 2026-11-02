import { describe, it, expect } from 'vitest';
import i18n from '@/i18n/config';
import {
  getSessionRuntimeActionBlockedReasonLabel,
  getSessionRunStatusLabel,
} from '@vibe/ui/lib/sessionRuntimeLabels';
describe('session action explanation boundary', () => {
  it('renders run status under the current locale without exposing unknown status data', () => {
    expect(
      getSessionRunStatusLabel('succeeded', i18n.getFixedT('en', 'tasks'))
    ).toBe('Completed');
    expect(
      getSessionRunStatusLabel('succeeded', i18n.getFixedT('ja', 'tasks'))
    ).toBe('完了');
    for (const status of [
      null,
      undefined,
      'provider-private',
      '__proto__',
      'toString',
    ]) {
      expect(
        getSessionRunStatusLabel(status, i18n.getFixedT('ja', 'tasks'))
      ).toBe('状態不明');
    }
  });
  it('keeps missing reasons absent and unknown data out of owned explanations', () => {
    for (const language of ['en', 'ja']) {
      const t = i18n.getFixedT(language, 'tasks');
      expect(getSessionRuntimeActionBlockedReasonLabel(null, t)).toBeNull();
      expect(
        getSessionRuntimeActionBlockedReasonLabel(undefined, t)
      ).toBeNull();
      for (const reason of ['Raw provider secret', 'toString', '__proto__'])
        expect(getSessionRuntimeActionBlockedReasonLabel(reason, t)).toBe(
          language === 'ja'
            ? '現在この操作は利用できません。'
            : 'This action is not available right now.'
        );
    }
  });
  it('resolves the same reason under the current translator without changing the reason code', () => {
    const reason = 'approval_timed_out';
    expect(
      getSessionRuntimeActionBlockedReasonLabel(
        reason,
        i18n.getFixedT('en', 'tasks')
      )
    ).toBe('The approval request timed out.');
    expect(
      getSessionRuntimeActionBlockedReasonLabel(
        reason,
        i18n.getFixedT('ja', 'tasks')
      )
    ).toBe('承認リクエストの期限が切れました。');
    expect(reason).toBe('approval_timed_out');
  });
});
