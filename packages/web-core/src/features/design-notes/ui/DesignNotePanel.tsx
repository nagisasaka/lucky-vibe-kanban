import { useTranslation } from 'react-i18next';
import { ApiError } from '@/shared/lib/api';
import { useId, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type {
  AdditionContent,
  DesignNote,
  DesignNoteAction,
  DesignNoteAddition,
  DesignNoteResult,
} from 'shared/types';
import WYSIWYGEditor from '@/shared/components/WYSIWYGEditor';
import { matchesListSearch } from '@/shared/lib/listSearch';
import { Textarea } from '@vibe/ui/components/Textarea';
import { designNoteApi as api } from '../model/designNoteApi';

const button =
  'rounded-sm border border-border px-base py-half text-sm text-normal hover:bg-secondary focus-visible:ring-1 focus-visible:ring-brand disabled:opacity-50';
const input =
  'w-full rounded-sm border border-border bg-primary p-base text-normal focus-visible:ring-1 focus-visible:ring-brand';
function useDesignNoteText() {
  const { t, i18n } = useTranslation('common');
  const kinds: Record<string, string> = {
    decision: t('designNotes.kinds.decision'),
    proposal: t('designNotes.kinds.proposal'),
    open_question: t('designNotes.kinds.openQuestion'),
  };
  const date = (value: string) =>
    new Date(value).toLocaleString(i18n.resolvedLanguage || i18n.language);
  return { t, kinds, date };
}
function Markdown({ value }: { value: string }) {
  return (
    <WYSIWYGEditor value={value} disabled className="break-words p-base" />
  );
}
function MarkdownInput({
  value,
  onChange,
  busy,
  label,
  placeholder,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  busy: boolean;
  label: string;
  placeholder: string;
  className: string;
}) {
  const { t } = useDesignNoteText();
  const id = useId();
  const [tab, setTab] = useState('edit');
  return (
    <div className="space-y-base">
      <div role="tablist" aria-label={label} className="flex gap-base">
        {(['edit', 'preview'] as const).map((mode) => (
          <button
            key={mode}
            id={`${id}-${mode}`}
            type="button"
            role="tab"
            aria-selected={tab === mode}
            aria-controls={`${id}-${mode}-panel`}
            tabIndex={tab === mode ? 0 : -1}
            className={`${button} ${tab === mode ? 'bg-secondary text-high' : ''}`}
            disabled={busy}
            onClick={() => setTab(mode)}
            onKeyDown={(e) => {
              if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key))
                return;
              e.preventDefault();
              const next =
                e.key === 'Home'
                  ? 'edit'
                  : e.key === 'End'
                    ? 'preview'
                    : mode === 'edit'
                      ? 'preview'
                      : 'edit';
              setTab(next);
              document.getElementById(`${id}-${next}`)?.focus();
            }}
          >
            {mode === 'edit' ? t('designNotes.edit') : t('designNotes.preview')}
          </button>
        ))}
      </div>
      <div
        id={`${id}-edit-panel`}
        role="tabpanel"
        aria-labelledby={`${id}-edit`}
        hidden={tab !== 'edit'}
      >
        <Textarea
          aria-label={label}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          disabled={busy}
          placeholder={placeholder}
          className={`${input} ${className} resize-y font-ibm-plex-mono`}
        />
      </div>
      <div
        id={`${id}-preview-panel`}
        role="tabpanel"
        aria-labelledby={`${id}-preview`}
        hidden={tab !== 'preview'}
        tabIndex={0}
        className="rounded-sm border border-border focus-visible:ring-1 focus-visible:ring-brand"
      >
        {tab === 'preview' && <Markdown value={value} />}
      </div>
    </div>
  );
}
type Write = (operation: DesignNoteAction) => Promise<DesignNoteResult>;
export function DesignNotePanel({ projectId }: { projectId: string }) {
  const { t, date } = useDesignNoteText();
  const client = useQueryClient();
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<
    'conflict' | 'invalid' | 'unconfirmed' | null
  >(null);
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  const active = useRef(false);
  // Retain an identical request ID after a lost response; changed input is a new intent.
  const request = useRef({ fingerprint: '', id: '' });
  const key = ['design-notes', projectId];
  const list = useQuery({
    queryKey: [...key, 'list'],
    queryFn: () => api.list(projectId),
    refetchInterval: 15000,
  });
  const detail = useQuery({
    queryKey: [...key, selected],
    queryFn: () => api.get(projectId, selected!),
    enabled: !!selected,
    refetchInterval: 15000,
  });
  const write: Write = async (operation) => {
    if (active.current) throw new Error(t('designNotes.saving'));
    active.current = true;
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    const fingerprint = JSON.stringify(operation);
    if (request.current.fingerprint !== fingerprint)
      request.current = { fingerprint, id: crypto.randomUUID() };
    try {
      const result = await api.write(projectId, {
        request_id: request.current.id,
        operation,
      });
      await client.invalidateQueries({ queryKey: key });
      request.current = { fingerprint: '', id: '' };
      return result;
    } catch (e) {
      setErrorDetail(e instanceof Error ? e.message : String(e));
      setError(
        e instanceof ApiError && e.status === 409
          ? 'conflict'
          : e instanceof ApiError && e.status === 400
            ? 'invalid'
            : 'unconfirmed'
      );
      // Refresh the comparison/current view immediately, without changing the
      // captured editor revision or draft. Reapplication remains explicit.
      await client.invalidateQueries({ queryKey: key });
      throw e;
    } finally {
      active.current = false;
      setBusy(false);
    }
  };
  return (
    <div
      data-testid="design-notes"
      className="flex h-full min-h-0 flex-col md:flex-row"
    >
      <aside
        className={`${selected || creating ? 'hidden md:block' : ''} w-full shrink-0 overflow-y-auto border-r border-border p-base md:w-64`}
      >
        <button
          className={button}
          onClick={() => {
            setSelected(null);
            setCreating(true);
          }}
          disabled={busy}
        >
          {t('designNotes.create')}
        </button>
        <input
          type="search"
          aria-label={t('designNotes.search')}
          placeholder={t('designNotes.searchPlaceholder')}
          className={`${input} mt-base`}
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        {list.error && (
          <p role="alert">
            {t('designNotes.errors.list')}{' '}
            {list.error instanceof Error
              ? list.error.message
              : String(list.error)}
          </p>
        )}
        {list.isLoading && <p>{t('designNotes.loading')}</p>}
        {list.data?.length === 0 && (
          <p className="p-base text-low">{t('designNotes.empty')}</p>
        )}
        {list.data
          ?.filter((n) => matchesListSearch(search, n.title, n.body))
          .map((n) => (
            <button
              key={n.id}
              className={`${button} mt-base w-full text-left break-words`}
              aria-pressed={selected === n.id}
              disabled={busy}
              onClick={() => {
                setSelected(n.id);
                setCreating(false);
                setError(null);
                setErrorDetail(null);
              }}
            >
              <span className="block">{n.title}</span>
              <time className="text-xs text-low">{date(n.updated_at)}</time>
            </button>
          ))}
      </aside>
      <main
        className={`${!selected && !creating ? 'hidden md:block' : ''} min-w-0 flex-1 overflow-y-auto p-double`}
      >
        <button
          className={`${button} mb-base md:hidden`}
          disabled={busy}
          onClick={() => {
            setSelected(null);
            setCreating(false);
          }}
        >
          {t('designNotes.back')}
        </button>
        {error && (
          <p role="alert" className="mb-base text-error">
            {error === 'conflict'
              ? t('designNotes.errors.conflict')
              : error === 'invalid'
                ? t('designNotes.errors.invalid')
                : t('designNotes.errors.unconfirmed')}{' '}
            {errorDetail}
          </p>
        )}
        {creating ? (
          <BodyForm
            busy={busy}
            onCancel={() => setCreating(false)}
            onSave={async (title, body) => {
              const r = await write({ action: 'create', title, body });
              setSelected(r.note.id);
              setCreating(false);
            }}
          />
        ) : selected ? (
          <>
            {detail.error && (
              <p role="alert">
                {t('designNotes.errors.detail')}{' '}
                {detail.error instanceof Error
                  ? detail.error.message
                  : String(detail.error)}
              </p>
            )}
            {detail.isLoading && <p>{t('designNotes.loading')}</p>}
            {detail.data && (
              <NoteDetail
                key={selected}
                projectId={projectId}
                note={detail.data.note}
                additions={detail.data.additions}
                write={write}
                busy={busy}
              />
            )}
          </>
        ) : (
          <p className="text-low">{t('designNotes.select')}</p>
        )}
      </main>
    </div>
  );
}
function BodyForm({
  note,
  onSave,
  onCancel,
  busy,
}: {
  note?: DesignNote;
  onSave: (title: string, body: string, reason: string) => Promise<void>;
  onCancel: () => void;
  busy: boolean;
}) {
  const { t } = useDesignNoteText();
  const [title, setTitle] = useState(note?.title ?? '');
  const [body, setBody] = useState(note?.body ?? '');
  const [reason, setReason] = useState('');
  return (
    <form
      className="space-y-base"
      onSubmit={(e) => {
        e.preventDefault();
        void onSave(title, body, reason).catch(() => {});
      }}
    >
      <label className="block">
        {t('designNotes.title')}
        <input
          className={input}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          maxLength={300}
          required
          disabled={busy}
        />
      </label>
      <div>
        <span>{t('designNotes.body')}</span>
        <div data-testid="note-body-editor">
          <MarkdownInput
            value={body}
            onChange={setBody}
            busy={busy}
            label={t('designNotes.body')}
            placeholder={t('designNotes.bodyPlaceholder')}
            className="min-h-40 p-base"
          />
        </div>
      </div>
      {note && (
        <label className="block">
          {t('designNotes.optionalReason')}
          <input
            className={input}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            maxLength={2000}
            disabled={busy}
          />
        </label>
      )}
      <div className="flex gap-base">
        <button className={button} disabled={busy || !title.trim()}>
          {t('designNotes.save')}
        </button>
        <button
          type="button"
          className={button}
          disabled={busy}
          onClick={onCancel}
        >
          {t('designNotes.cancel')}
        </button>
      </div>
    </form>
  );
}
function NoteDetail({
  projectId,
  note,
  additions,
  write,
  busy,
}: {
  projectId: string;
  note: DesignNote;
  additions: DesignNoteAddition[];
  write: Write;
  busy: boolean;
}) {
  const { t, date } = useDesignNoteText();
  // Capture the version being edited. Background refresh must never silently rebase a draft.
  const [editing, setEditing] = useState<DesignNote | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [viewed, setViewed] = useState<DesignNote | null>(null);
  const [showHidden, setShowHidden] = useState(false);
  const [adding, setAdding] = useState(false);
  const history = useQuery({
    queryKey: ['design-notes', projectId, note.id, 'history'],
    queryFn: () => api.history(projectId, note.id),
    enabled: historyOpen,
  });
  const shown = viewed ?? note;
  if (editing)
    return (
      <div className="space-y-double">
        <BodyForm
          note={editing}
          busy={busy}
          onCancel={() => setEditing(null)}
          onSave={async (title, body, reason) => {
            await write({
              action: 'update',
              note_id: note.id,
              expected_revision: editing.revision,
              title,
              body,
              change_reason: reason || null,
            });
            setEditing(null);
          }}
        />
        {note.revision !== editing.revision && (
          <section
            className="space-y-base border border-border p-base"
            data-testid="body-conflict-current"
          >
            <p>
              {t('designNotes.currentBody', {
                revision: note.revision,
                title: note.title,
              })}
            </p>
            <Markdown value={note.body} />
            <p className="text-sm text-low">
              {t('designNotes.bodyConflictHint')}
            </p>
            <button
              className={button}
              disabled={busy}
              onClick={() => setEditing(note)}
            >
              {t('designNotes.reapplyBody')}
            </button>
          </section>
        )}
      </div>
    );
  return (
    <div className="space-y-double">
      <header className="space-y-base">
        <h2 className="text-xl text-high break-words">{shown.title}</h2>
        <p className="text-xs text-low break-all">
          {t('designNotes.bodyMetadata', {
            version: viewed
              ? t('designNotes.pastVersion')
              : t('designNotes.currentVersion'),
            revision: shown.revision,
            date: date(shown.updated_at),
            editor: shown.editor,
          })}
        </p>
        {shown.change_reason && (
          <p>
            {t('designNotes.changeReason', { reason: shown.change_reason })}
          </p>
        )}
        <div className="flex flex-wrap gap-base">
          <button
            className={button}
            disabled={busy}
            onClick={() => {
              setViewed(null);
              setEditing(note);
            }}
          >
            {t('designNotes.editBody')}
          </button>
          <button
            className={button}
            onClick={() => setHistoryOpen(!historyOpen)}
          >
            {t('designNotes.history')}
          </button>
          {viewed && (
            <>
              <button className={button} onClick={() => setViewed(null)}>
                {t('designNotes.showCurrent')}
              </button>
              <button
                className={button}
                disabled={busy}
                onClick={() =>
                  void write({
                    action: 'restore',
                    note_id: note.id,
                    expected_revision: note.revision,
                    revision: viewed.revision,
                    change_reason: null,
                  })
                    .then(() => setViewed(null))
                    .catch(() => {})
                }
              >
                {t('designNotes.restore')}
              </button>
            </>
          )}
        </div>
        {historyOpen && (
          <div className="space-y-base rounded-sm border border-border p-base">
            <p className="text-xs text-low">
              {t('designNotes.firstVersionHint')}
            </p>
            {history.error && (
              <p role="alert">
                {t('designNotes.errors.history')}{' '}
                {history.error instanceof Error
                  ? history.error.message
                  : String(history.error)}
              </p>
            )}
            <div className="flex flex-wrap gap-base">
              {history.data?.map((n) => (
                <button
                  key={n.revision}
                  className={button}
                  onClick={() => setViewed(n)}
                >
                  {n.revision === 1
                    ? t('designNotes.firstVersion')
                    : t('designNotes.revision', { revision: n.revision })}{' '}
                  · {date(n.updated_at)}
                </button>
              ))}
            </div>
          </div>
        )}
      </header>
      <section
        data-testid="note-body"
        className="rounded-sm border border-border"
      >
        <Markdown value={shown.body} />
      </section>
      <section className="space-y-base border-t border-border pt-double">
        <h3 className="text-lg text-high">{t('designNotes.additions')}</h3>
        <p className="text-xs text-low">{t('designNotes.additionsHint')}</p>
        <div className="flex flex-wrap gap-base">
          <button
            className={button}
            disabled={busy}
            onClick={() => setAdding(!adding)}
          >
            {t('designNotes.append')}
          </button>
          <label>
            <input
              type="checkbox"
              checked={showHidden}
              onChange={(e) => setShowHidden(e.target.checked)}
            />{' '}
            {t('designNotes.showHidden')}
          </label>
        </div>
        {adding && (
          <AdditionForm
            revision={note.revision}
            busy={busy}
            onCancel={() => setAdding(false)}
            onSave={async (addition) => {
              await write({ action: 'append', note_id: note.id, addition });
              setAdding(false);
            }}
          />
        )}
        {additions.map((a) => (
          <Addition
            key={a.id}
            projectId={projectId}
            note={note}
            addition={a}
            showHidden={showHidden}
            write={write}
            busy={busy}
          />
        ))}
      </section>
    </div>
  );
}
function AdditionText({ addition: a }: { addition: DesignNoteAddition }) {
  const { t, kinds, date } = useDesignNoteText();
  return (
    <>
      <h4 className="text-high break-words">
        {a.heading}
        {a.hidden && t('designNotes.hidden')}
      </h4>
      <p className="text-sm">{kinds[a.kind] ?? a.kind}</p>
      <Markdown value={a.content} />
      <dl className="space-y-half text-xs text-low break-all">
        <dt>{t('designNotes.speakerOrigin')}</dt>
        <dd>{a.speaker}</dd>
        <dt>{t('designNotes.source')}</dt>
        <dd>{a.source}</dd>
        <dt>{t('designNotes.recorder')}</dt>
        <dd>{a.recorder}</dd>
        <dt>{t('designNotes.referencedVersion')}</dt>
        <dd>
          {t('designNotes.revision', { revision: a.referenced_body_revision })}
        </dd>
        <dt>{t('designNotes.recordedUpdated')}</dt>
        <dd>
          {t('designNotes.additionMetadata', {
            created: date(a.created_at),
            updated: date(a.updated_at),
            editor: a.editor,
            revision: a.revision,
          })}
        </dd>
      </dl>
      {a.short_quote && (
        <blockquote className="border-l border-border p-base">
          {a.short_quote}
        </blockquote>
      )}
    </>
  );
}
function Addition({
  projectId,
  note,
  addition: a,
  showHidden,
  write,
  busy,
}: {
  projectId: string;
  note: DesignNote;
  addition: DesignNoteAddition;
  showHidden: boolean;
  write: Write;
  busy: boolean;
}) {
  const { t } = useDesignNoteText();
  const [editing, setEditing] = useState<DesignNoteAddition | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const history = useQuery({
    queryKey: ['design-notes', projectId, note.id, a.id, 'history'],
    queryFn: () => api.additionHistory(projectId, note.id, a.id),
    enabled: historyOpen,
  });
  // Keep the component mounted across remote hiding so an active draft survives.
  if (a.hidden && !showHidden && !editing) return null;
  return (
    <article
      data-testid="note-addition"
      className="space-y-base rounded-sm border border-border p-base"
    >
      <AdditionText addition={a} />
      <div className="flex flex-wrap gap-base">
        <button
          className={button}
          disabled={busy || !!editing}
          onClick={() => setEditing(a)}
        >
          {t('designNotes.correctAddition')}
        </button>
        <button
          className={button}
          disabled={busy || !!editing}
          onClick={() =>
            void write({
              action: 'set_addition_hidden',
              note_id: note.id,
              addition_id: a.id,
              expected_revision: a.revision,
              hidden: !a.hidden,
            }).catch(() => {})
          }
        >
          {a.hidden ? t('designNotes.unhide') : t('designNotes.hide')}
        </button>
        <button className={button} onClick={() => setHistoryOpen(!historyOpen)}>
          {t('designNotes.additionHistory')}
        </button>
        <button
          className={button}
          disabled={busy || a.hidden || !!editing}
          onClick={() => {
            if (window.confirm(t('designNotes.integrateConfirm')))
              void write({
                action: 'integrate',
                note_id: note.id,
                expected_revision: note.revision,
                addition_id: a.id,
                expected_addition_revision: a.revision,
                change_reason: null,
              }).catch(() => {});
          }}
        >
          {t('designNotes.integrate')}
        </button>
      </div>
      {editing && (
        <AdditionForm
          initial={editing}
          revision={editing.referenced_body_revision}
          busy={busy}
          onCancel={() => setEditing(null)}
          onSave={async (addition) => {
            await write({
              action: 'correct_addition',
              note_id: note.id,
              addition_id: a.id,
              expected_revision: editing.revision,
              addition,
            });
            setEditing(null);
          }}
        />
      )}
      {editing && editing.revision !== a.revision && (
        <div
          className="space-y-base border border-border p-base"
          data-testid="addition-conflict-current"
        >
          <p>{t('designNotes.additionConflict', { revision: a.revision })}</p>
          <button
            className={button}
            disabled={busy}
            onClick={() => setEditing(a)}
          >
            {t('designNotes.reapplyAddition')}
          </button>
        </div>
      )}
      {historyOpen && (
        <div className="space-y-base">
          {history.error && (
            <p role="alert">
              {t('designNotes.errors.history')}{' '}
              {history.error instanceof Error
                ? history.error.message
                : String(history.error)}
            </p>
          )}
          {history.data?.map((old) => (
            <div className="border-t border-border pt-base" key={old.revision}>
              <AdditionText addition={old} />
            </div>
          ))}
        </div>
      )}
    </article>
  );
}
function AdditionForm({
  initial,
  revision,
  busy,
  onSave,
  onCancel,
}: {
  initial?: DesignNoteAddition;
  revision: number;
  busy: boolean;
  onSave: (a: AdditionContent) => Promise<void>;
  onCancel: () => void;
}) {
  const { t, kinds } = useDesignNoteText();
  const [value, setValue] = useState<AdditionContent>({
    heading: initial?.heading ?? '',
    content: initial?.content ?? '',
    kind: initial?.kind ?? 'open_question',
    speaker: initial?.speaker ?? '',
    source: initial?.source ?? '',
    referenced_body_revision: revision,
    short_quote: initial?.short_quote ?? null,
  });
  return (
    <form
      className="space-y-base rounded-sm border border-border p-base"
      onSubmit={(e) => {
        e.preventDefault();
        void onSave(value).catch(() => {});
      }}
    >
      {(['heading', 'speaker', 'source'] as const).map((field) => (
        <label key={field} className="block">
          {
            {
              heading: t('designNotes.heading'),
              speaker: t('designNotes.speaker'),
              source: t('designNotes.sourceReference'),
            }[field]
          }
          <input
            className={input}
            value={value[field]}
            disabled={busy}
            required
            maxLength={field === 'heading' ? 300 : 2000}
            onChange={(e) => setValue({ ...value, [field]: e.target.value })}
          />
        </label>
      ))}
      <label className="block">
        {t('designNotes.kind')}
        <select
          className={input}
          value={value.kind}
          disabled={busy}
          onChange={(e) => setValue({ ...value, kind: e.target.value })}
        >
          {Object.entries(kinds).map(([key, label]) => (
            <option key={key} value={key}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <div data-testid="addition-editor">
        <MarkdownInput
          value={value.content}
          onChange={(content) => setValue({ ...value, content })}
          busy={busy}
          label={t('designNotes.additionContent')}
          placeholder={t('designNotes.additionPlaceholder')}
          className="min-h-24 p-base"
        />
      </div>
      <label className="block">
        {t('designNotes.referencedRevision')}
        <input
          type="number"
          min={1}
          required
          className={input}
          value={value.referenced_body_revision}
          disabled={busy}
          onChange={(e) =>
            setValue({
              ...value,
              referenced_body_revision: Number(e.target.value),
            })
          }
        />
      </label>
      <label className="block">
        {t('designNotes.optionalQuote')}
        <input
          className={input}
          value={value.short_quote ?? ''}
          maxLength={2000}
          disabled={busy}
          onChange={(e) =>
            setValue({ ...value, short_quote: e.target.value || null })
          }
        />
      </label>
      <div className="flex gap-base">
        <button className={button} disabled={busy || !value.content.trim()}>
          {t('designNotes.saveAddition')}
        </button>
        <button
          type="button"
          className={button}
          disabled={busy}
          onClick={onCancel}
        >
          {t('designNotes.cancel')}
        </button>
      </div>
    </form>
  );
}
