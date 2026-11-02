import i18n from '@/i18n/config';
import type { Workspace } from 'shared/types';
import { Pages, getPageActions } from '@/shared/command-bar/actions/pages';
import type { StaticPageId, ResolvedGroup } from '@/shared/types/commandBar';
import {
  getActionLabel,
  type ActionTranslator,
  isActionVisible,
  type ActionVisibilityContext,
} from '@/shared/types/actions';

// Derive injectable pages from Pages - all child pages of root
const INJECTABLE_PAGE_IDS = (Object.keys(Pages) as StaticPageId[]).filter(
  (id) => id !== 'root' && Pages[id].parent === 'root'
);

export function injectSearchMatches(
  searchQuery: string,
  ctx: ActionVisibilityContext,
  workspace: Workspace | undefined,
  t: ActionTranslator = i18n.getFixedT(null, 'tasks')
): ResolvedGroup[] {
  const searchLower = searchQuery.toLowerCase();

  return INJECTABLE_PAGE_IDS.reduce<ResolvedGroup[]>((groups, id) => {
    const page = Pages[id];

    // Check page visibility condition
    if (page.isVisible && !page.isVisible(ctx)) return groups;

    const items = getPageActions(id)
      .filter((a) => isActionVisible(a, ctx))
      .filter((a) => {
        const label = getActionLabel(a, ctx, workspace, t);
        return (
          label.toLowerCase().includes(searchLower) ||
          a.id.toLowerCase().includes(searchLower) ||
          (a.keywords?.some((kw) => kw.toLowerCase().includes(searchLower)) ??
            false)
        );
      })
      .map((action) => ({ type: 'action' as const, action }));

    if (items.length) groups.push({ label: page.title?.(t) || id, items });
    return groups;
  }, []);
}
