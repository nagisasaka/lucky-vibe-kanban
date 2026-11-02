import type { TFunction } from 'i18next';
import type { GroupedNotification } from '@/shared/lib/notifications';
import { getPayload } from '@/shared/lib/notifications';
import type { OrganizationMemberWithProfile } from 'shared/types';

export type MessageSegment =
  | { type: 'text'; value: string }
  | { type: 'emphasis'; value: string }
  | { type: 'issue'; value: string }
  | { type: 'user'; userId: string };

const text = (value: string): MessageSegment => ({ type: 'text', value });
const emphasis = (value: string): MessageSegment => ({
  type: 'emphasis',
  value,
});
const user = (userId: string): MessageSegment => ({ type: 'user', userId });

function getMemberLabel(member?: OrganizationMemberWithProfile): string | null {
  if (!member) return null;
  const fullName = [member.first_name, member.last_name]
    .filter((value): value is string => Boolean(value && value.trim()))
    .join(' ');
  return fullName || (member.username?.trim() ? member.username : null);
}

// Translate sentence order before inserting raw segments. Raw payload values are
// never parsed as templates or markup, even if they resemble interpolation tags.
function compose(
  template: string,
  values: Record<string, MessageSegment[]>
): MessageSegment[] {
  return template
    .split(/(\uE000[a-z]+\uE001)/)
    .filter(Boolean)
    .flatMap((part) =>
      part.startsWith('\uE000')
        ? (values[part.slice(1, -1)] ?? [text(part)])
        : [text(part)]
    );
}

export function getGroupedNotificationSegments(
  group: GroupedNotification,
  t: TFunction<'common'>,
  locale: string
): MessageSegment[] {
  const payload = getPayload(group.latest);
  const someone = () => [text(t('notifications.someone'))];
  const actor = payload.actor_user_id
    ? [user(payload.actor_user_id)]
    : someone();
  const issue: MessageSegment[] = payload.issue_simple_id
    ? [{ type: 'issue', value: payload.issue_simple_id }]
    : [emphasis(payload.issue_title ?? t('notifications.anIssue'))];
  const tokens = {
    actor: '\uE000actor\uE001',
    issue: '\uE000issue\uE001',
    assignee: '\uE000assignee\uE001',
    old: '\uE000old\uE001',
    next: '\uE000next\uE001',
    quantity: '\uE000quantity\uE001',
    emoji: '\uE000emoji\uE001',
  };
  const render = (
    template: string,
    values: Record<string, MessageSegment[]> = {}
  ) => compose(template, { actor, issue, ...values });
  const countOptions = (count: number) => ({
    count,
    formattedCount: new Intl.NumberFormat(locale).format(count),
  });
  const priorities: Record<string, string> = {
    urgent: t('notifications.priorityUrgent'),
    high: t('notifications.priorityHigh'),
    medium: t('notifications.priorityMedium'),
    low: t('notifications.priorityLow'),
  };
  const priority = (value: string) =>
    Object.prototype.hasOwnProperty.call(priorities, value)
      ? priorities[value]
      : value;

  if (group.kind !== 'single' && group.notificationCount > 1) {
    switch (group.kind) {
      case 'issue_changes':
        return render(t('notifications.groupChanges', tokens), {
          quantity: [
            emphasis(
              t('notifications.fields', countOptions(group.issueChangeCount))
            ),
          ],
        });
      case 'comments':
        return render(t('notifications.groupComments', tokens), {
          quantity: [
            emphasis(
              t('notifications.comments', countOptions(group.notificationCount))
            ),
          ],
        });
      case 'status_changes':
        return render(t('notifications.statusChanged', tokens));
      case 'reactions':
        return render(t('notifications.groupReactions', tokens), {
          quantity: [
            emphasis(
              t('notifications.times', countOptions(group.notificationCount))
            ),
          ],
        });
      case 'issue_deleted':
        return render(t('notifications.groupDeleted', tokens), {
          quantity: [
            emphasis(
              t('notifications.issues', countOptions(group.notificationCount))
            ),
          ],
        });
    }
  }
  switch (group.latest.notification_type) {
    case 'issue_title_changed':
      return payload.new_title
        ? render(t('notifications.titleChangedTo', tokens), {
            next: [emphasis(payload.new_title)],
          })
        : render(t('notifications.titleChanged', tokens));
    case 'issue_assignee_changed':
      return render(t('notifications.assigned', tokens), {
        assignee: payload.assignee_user_id
          ? [user(payload.assignee_user_id)]
          : someone(),
      });
    case 'issue_unassigned':
      return render(t('notifications.unassigned', tokens));
    case 'issue_description_changed':
      return render(t('notifications.descriptionChanged', tokens));
    case 'issue_priority_changed': {
      const old = payload.old_priority;
      const next = payload.new_priority;
      if (old && next)
        return render(t('notifications.priorityChangedFrom', tokens), {
          old: [emphasis(priority(old))],
          next: [emphasis(priority(next))],
        });
      if (next)
        return render(t('notifications.priorityChangedTo', tokens), {
          next: [emphasis(priority(next))],
        });
      return render(t('notifications.priorityCleared', tokens));
    }
    case 'issue_comment_added':
      return render(t('notifications.commented', tokens));
    case 'issue_comment_reaction':
      return payload.emoji
        ? render(t('notifications.reactedWith', tokens), {
            emoji: [emphasis(payload.emoji)],
          })
        : render(t('notifications.reacted', tokens));
    case 'issue_status_changed':
      return payload.old_status_name && payload.new_status_name
        ? render(t('notifications.statusChangedFrom', tokens), {
            old: [emphasis(payload.old_status_name)],
            next: [emphasis(payload.new_status_name)],
          })
        : render(t('notifications.statusChanged', tokens));
    case 'issue_deleted':
      return render(t('notifications.deleted', tokens));
    default:
      return [text(t('notifications.newNotification'))];
  }
}

export function getGroupedNotificationText(
  group: GroupedNotification,
  t: TFunction<'common'>,
  locale: string,
  membersByUserId?: Map<string, OrganizationMemberWithProfile>
): string {
  return getGroupedNotificationSegments(group, t, locale)
    .map((segment) =>
      segment.type === 'user'
        ? (getMemberLabel(membersByUserId?.get(segment.userId)) ??
          t('notifications.someone'))
        : segment.value
    )
    .join('');
}
