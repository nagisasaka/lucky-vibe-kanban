import { describe, it, expect } from 'vitest';
import i18n from '@/i18n/config';
import type {
  Notification,
  NotificationPayload,
  NotificationType,
} from 'shared/remote-types';
import type { OrganizationMemberWithProfile } from 'shared/types';
import { groupNotifications, type GroupedNotification } from './notifications';
import {
  getGroupedNotificationSegments,
  getGroupedNotificationText,
} from './notificationMessage';
const row = (
  type: NotificationType,
  payload: NotificationPayload = {}
): Notification => ({
  id: 'raw-notification',
  organization_id: 'raw-org',
  user_id: 'raw-recipient',
  notification_type: type,
  payload: {
    actor_user_id: 'raw-actor',
    issue_simple_id: 'RAW-12',
    ...payload,
  },
  issue_id: 'raw-issue',
  comment_id: null,
  seen: false,
  dismissed_at: null,
  created_at: '2026-11-01T12:00:00Z',
});
const group = (
  type: NotificationType,
  payload: NotificationPayload = {}
): GroupedNotification => groupNotifications([row(type, payload)])[0];
const members = new Map([
  [
    'raw-actor',
    {
      first_name: 'Alex 原文',
      last_name: null,
    } as OrganizationMemberWithProfile,
  ],
  ['raw-assignee', { username: 'User 原文' } as OrganizationMemberWithProfile],
]);
const cases: [NotificationType, NotificationPayload, string, string][] = [
  [
    'issue_title_changed',
    { new_title: 'Raw title 原文' },
    'Alex 原文 changed the title of RAW-12 to Raw title 原文',
    'Alex 原文がRAW-12のタイトルを「Raw title 原文」に変更しました',
  ],
  [
    'issue_title_changed',
    {},
    'Alex 原文 changed the title of RAW-12',
    'Alex 原文がRAW-12のタイトルを変更しました',
  ],
  [
    'issue_assignee_changed',
    { assignee_user_id: 'raw-assignee' },
    'User 原文 was assigned to RAW-12 by Alex 原文',
    'Alex 原文がRAW-12をUser 原文に割り当てました',
  ],
  [
    'issue_unassigned',
    {},
    'Alex 原文 unassigned you from RAW-12',
    'Alex 原文がRAW-12のあなたへの割り当てを解除しました',
  ],
  [
    'issue_description_changed',
    {},
    'Alex 原文 changed the description on RAW-12',
    'Alex 原文がRAW-12の説明を変更しました',
  ],
  [
    'issue_priority_changed',
    { old_priority: 'low', new_priority: 'high' },
    'Alex 原文 changed the priority of RAW-12 from Low to High',
    'Alex 原文がRAW-12の優先度を「低」から「高」に変更しました',
  ],
  [
    'issue_priority_changed',
    { new_priority: 'urgent' },
    'Alex 原文 changed the priority of RAW-12 to Urgent',
    'Alex 原文がRAW-12の優先度を「緊急」に変更しました',
  ],
  [
    'issue_priority_changed',
    {},
    'Alex 原文 cleared the priority of RAW-12',
    'Alex 原文がRAW-12の優先度を解除しました',
  ],
  [
    'issue_comment_added',
    {},
    'Alex 原文 commented on RAW-12',
    'Alex 原文がRAW-12にコメントしました',
  ],
  [
    'issue_comment_reaction',
    { emoji: '🚀' },
    'Alex 原文 reacted 🚀 to your comment on RAW-12',
    'Alex 原文がRAW-12のあなたのコメントに🚀でリアクションしました',
  ],
  [
    'issue_comment_reaction',
    {},
    'Alex 原文 reacted to your comment on RAW-12',
    'Alex 原文がRAW-12のあなたのコメントにリアクションしました',
  ],
  [
    'issue_status_changed',
    { old_status_name: 'Raw Open 原文', new_status_name: 'Raw Done 原文' },
    'Alex 原文 changed status of RAW-12 from Raw Open 原文 to Raw Done 原文',
    'Alex 原文がRAW-12のステータスを「Raw Open 原文」から「Raw Done 原文」に変更しました',
  ],
  [
    'issue_status_changed',
    {},
    'Alex 原文 changed status of RAW-12',
    'Alex 原文がRAW-12のステータスを変更しました',
  ],
  [
    'issue_deleted',
    {},
    'Alex 原文 deleted RAW-12',
    'Alex 原文がRAW-12を削除しました',
  ],
];
for (const language of ['en', 'ja']) {
  describe(`notification sentences ${language}`, () => {
    const t = i18n.getFixedT(language, 'common');
    for (const [type, payload, en, ja] of cases)
      it(`${type} ${JSON.stringify(payload)}`, () => {
        const value = group(type, payload);
        const before = JSON.stringify(value);
        expect(getGroupedNotificationText(value, t, language, members)).toBe(
          language === 'ja' ? ja : en
        );
        expect(JSON.stringify(value)).toBe(before);
      });
    it('uses localized fallbacks without altering raw titles or parsing them as templates', () => {
      const raw = '<b>{{actor}}</b> \uE000actor\uE001';
      const value = group('issue_title_changed', {
        actor_user_id: null,
        issue_simple_id: null,
        issue_title: raw,
        new_title: raw,
      });
      const segments = getGroupedNotificationSegments(value, t, language);
      expect(
        segments.filter((s) => s.type === 'emphasis').map((s) => s.value)
      ).toEqual([raw, raw]);
      const fallback = getGroupedNotificationText(
        group('issue_deleted', {
          actor_user_id: null,
          issue_simple_id: null,
          issue_title: null,
        }),
        t,
        language
      );
      expect(fallback).toBe(
        language === 'ja'
          ? '誰かが課題を削除しました'
          : 'Someone deleted an issue'
      );
    });
    it('localizes grouped counts and preserves single-field/plural distinctions', () => {
      for (const [kind, count, en, ja] of [
        ['issue_changes', 1, '1 field', '1項目'],
        ['issue_changes', 2, '2 fields', '2項目'],
        ['comments', 1000, '1,000 comments', '1,000件のコメント'],
        ['reactions', 2, '2 times', '2回'],
        ['issue_deleted', 2, '2 issues', '2件の課題'],
      ] as const) {
        const value = {
          ...group('issue_comment_added'),
          kind,
          notificationCount: kind === 'comments' ? count : 2,
          issueChangeCount: count,
        };
        expect(
          getGroupedNotificationText(value, t, language, members)
        ).toContain(language === 'ja' ? ja : en);
      }
    });
    it('retains future priority values and safely handles unknown event types', () => {
      const value = group('issue_priority_changed', {
        new_priority: 'toString' as 'low',
      });
      expect(getGroupedNotificationText(value, t, language)).toContain(
        'toString'
      );
      const unknown = group('future_event' as NotificationType);
      expect(getGroupedNotificationText(unknown, t, language)).toBe(
        language === 'ja' ? '新しい通知' : 'New notification'
      );
    });
  });
}
