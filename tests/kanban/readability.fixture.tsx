import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import common from '../../packages/web-core/src/i18n/locales/en/common.json';
import { KanbanCardContent } from '../../packages/ui/src/components/KanbanCardContent';
import {
  IssueWorkspaceCard,
  type WorkspaceWithStats,
} from '../../packages/ui/src/components/IssueWorkspaceCard';
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from '../../packages/ui/src/components/Popover';
import { TooltipProvider } from '../../packages/ui/src/components/RadixTooltip';
import { KanbanCardDetails } from '../../packages/web-core/src/features/kanban/ui/KanbanCardDetails';
import {
  KanbanGrid,
  KanbanGridCards,
  KanbanGridCard,
} from '../../packages/web-core/src/features/kanban/ui/KanbanGrid';

i18n
  .use(initReactI18next)
  .init({ lng: 'en', resources: { en: { common } }, initImmediate: false });
const description =
  '# Specification\n**Readable** [link](https://example.invalid)\n```ts\nlet value = 1;\n```\n![photo.png](attachment://1)\nhttps://example.invalid/' +
  'long-url'.repeat(60);
const start = new Date(Date.now() - 65000).toISOString();
const workspace = (n: number): WorkspaceWithStats => ({
  id: String(n),
  localWorkspaceId: String(n),
  name: `Workspace ${n}`,
  archived: false,
  filesChanged: n === 2 ? undefined : n,
  linesAdded: n === 2 || n === 3 ? undefined : n * 100000,
  linesRemoved: n === 2 || n === 3 ? undefined : n * 200000,
  prs: [],
  owner: null,
  updatedAt: new Date().toISOString(),
  isOwnedByCurrentUser: true,
  latestProcessStartedAt: n === 2 ? undefined : start,
  latestProcessCompletedAt:
    n === 3 ? new Date(Date.parse(start) + 32000).toISOString() : undefined,
  latestProcessStatus:
    n === 0
      ? 'running'
      : n === 1
        ? 'awaiting_approval'
        : n === 3
          ? 'succeeded'
          : n === 4 ? 'awaiting_input' : 'failed',
  isRunning: n < 2 || n === 4,
  hasPendingApproval: n === 1,
  hasUnseenActivity: n === 2,
  hasRunningDevServer: n < 3 || n === 4,
});
function Fixture() {
  const [columns, setColumns] = useState([
    ['a', 'b', 'c'],
    ['d'],
    ['e'],
    [],
  ] as string[][]);
  const [show, setShow] = useState(true);
  const [revision, update] = useState(0);
  const [opened, open] = useState('');
  const [tagCount, setTagCount] = useState(3);
  const [longTags, setLongTags] = useState(true);
  const tags = Array.from({ length: tagCount }, (_, n) => ({
    id: String(n),
    name: n === 0 && longTags ? 'Long label '.repeat(12) : `Tag ${n}`,
    color: '120 60% 40%',
  }));
  return (
    <TooltipProvider>
      <button onClick={() => setShow(!show)}>Toggle workspaces</button>
      <button onClick={() => update(revision + 1)}>Summary update</button>
      <button onClick={() => setTagCount((tagCount + 1) % 5)}>
        Change tags
      </button>
      <button onClick={() => setLongTags(!longTags)}>Toggle long tags</button>
      <output>{opened}</output>
      <main className="bg-primary text-normal">
        <KanbanGrid
          onDragEnd={({ source, destination, draggableId }) => {
            if (!destination) return;
            const next = columns.map((c) =>
              c.filter((id) => id !== draggableId)
            );
            next[Number(destination.droppableId)].splice(
              destination.index,
              0,
              draggableId
            );
            setColumns(next);
          }}
        >
          {columns.map((ids, column) => (
            <section key={column} className="flex flex-col min-w-0">
              <h2>
                {['Not Started', 'In Progress', 'In Review', 'Done'][column]}
              </h2>
              <KanbanGridCards id={String(column)} items={ids}>
                {ids.map((id) => {
                  const workspaces = Array.from(
                    {
                      length:
                        id === 'a' ? 4 : id === 'b' ? 12 : id === 'd' ? 1 : 0,
                    },
                    (_, n) => workspace(n)
                  );
                  return (
                    <KanbanGridCard
                      key={id}
                      id={id}
                      name={id}
                      onClick={() => open(`Issue ${id}`)}
                    >
                      <KanbanCardContent
                        displayId={id === 'a' ? 'LOCAL-VERY-LONG-PROJECT-ID-123456789' : id}
                        title={'Long readable title '.repeat(9)}
                        description={id === 'c' ? '   ' : description}
                        tags={tags}
                        tagEditProps={{
                          allTags: tags,
                          selectedTagIds: tags.map((t) => t.id),
                          onTagToggle: () => {},
                          onCreateTag: () => '',
                          renderTagEditor: ({ trigger }) => (
                            <Popover>
                              <PopoverTrigger asChild>{trigger}</PopoverTrigger>
                              <PopoverContent>Tag editor</PopoverContent>
                            </Popover>
                          ),
                        }}
                        priority="high"
                        onPriorityClick={(event) => { event.stopPropagation(); open('Priority'); }}
                        onAssigneeClick={(event) => { event.stopPropagation(); open('Assignees'); }}
                        assignees={[{user_id:'one', username:'Alice'}, {user_id:'two', username:'Bob'}, {user_id:'three', username:'Carol'}]}
                      />
                      <KanbanCardDetails
                        count={workspaces.length}
                        description={id === 'c' ? '' : description}
                      />
                      {show && (
                        <div className="mt-base flex flex-col gap-half">
                          {workspaces.map((w) => (
                            <IssueWorkspaceCard
                              key={w.id}
                              workspace={w}
                              onClick={() => open(`Workspace ${w.id}`)}
                              showOwner={false}
                              showStatusBadge={false}
                              showNoPrText={false}
                            />
                          ))}
                        </div>
                      )}
                    </KanbanGridCard>
                  );
                })}
              </KanbanGridCards>
            </section>
          ))}
        </KanbanGrid>
      </main>
    </TooltipProvider>
  );
}
createRoot(document.getElementById('root')!).render(<Fixture />);
