import { useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type {
  AdditionContent,
  DesignNote,
  DesignNoteAction,
  DesignNoteAddition,
  DesignNoteResult,
} from 'shared/types';
import WYSIWYGEditor from '@/shared/components/WYSIWYGEditor';
import { designNoteApi as api } from '../model/designNoteApi';

const button =
  'rounded-sm border border-border px-base py-half text-sm text-normal hover:bg-secondary focus-visible:ring-1 focus-visible:ring-brand disabled:opacity-50';
const input =
  'w-full rounded-sm border border-border bg-primary p-base text-normal focus-visible:ring-1 focus-visible:ring-brand';
const kinds: Record<string, string> = {
  decision: '確定事項（出典のある人間の判断）',
  proposal: '提案',
  open_question: '未決事項',
};
const date = (value: string) => new Date(value).toLocaleString();
function Markdown({ value }: { value: string }) {
  return (
    <WYSIWYGEditor value={value} disabled className="break-words p-base" />
  );
}
type Write = (operation: DesignNoteAction) => Promise<DesignNoteResult>;
export function DesignNotePanel({ projectId }: { projectId: string }) {
  const client = useQueryClient();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const active = useRef(false);
  // Retain an identical request ID after a lost response; changed input is a new intent.
  const request = useRef({ fingerprint: '', id: '' });
  const key = ['design-notes', projectId];
  const list = useQuery({
    queryKey: [...key, 'list'],
    queryFn: () => api.list(projectId),
    refetchInterval: 5000,
  });
  const detail = useQuery({
    queryKey: [...key, selected],
    queryFn: () => api.get(projectId, selected!),
    enabled: !!selected,
    refetchInterval: 5000,
  });
  const write: Write = async (operation) => {
    if (active.current) throw new Error('保存中です');
    active.current = true;
    setBusy(true);
    setError('');
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
      setError(
        `${e instanceof Error ? e.message : String(e)}。入力は保持しています。最新の内容を確認してから編集し直してください。`
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
          新しい設計メモ
        </button>
        {list.error && <p role="alert">{String(list.error)}</p>}
        {list.isLoading && <p>読み込み中…</p>}
        {list.data?.length === 0 && (
          <p className="p-base text-low">
            最初の考えと、その後の判断を残せます。
          </p>
        )}
        {list.data?.map((n) => (
          <button
            key={n.id}
            className={`${button} mt-base w-full text-left break-words`}
            aria-pressed={selected === n.id}
            disabled={busy}
            onClick={() => {
              setSelected(n.id);
              setCreating(false);
              setError('');
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
          一覧へ
        </button>
        {error && (
          <p role="alert" className="mb-base text-error">
            {error}
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
            {detail.error && <p role="alert">{String(detail.error)}</p>}
            {detail.isLoading && <p>読み込み中…</p>}
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
          <p className="text-low">設計メモを選択してください。</p>
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
        タイトル
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
        <span>本文（Markdown）</span>
        <div
          data-testid="note-body-editor"
          className="rounded-sm border border-border"
        >
          <WYSIWYGEditor
            value={body}
            onChange={setBody}
            disabled={busy}
            placeholder="何を作りたいか、なぜそう考えたか"
            className="min-h-40 p-base"
          />
        </div>
      </div>
      {note && (
        <label className="block">
          変更理由（任意）
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
          保存
        </button>
        <button
          type="button"
          className={button}
          disabled={busy}
          onClick={onCancel}
        >
          キャンセル
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
              保存済みの現在版（revision {note.revision}）: {note.title}
            </p>
            <Markdown value={note.body} />
            <p className="text-sm text-low">
              上の下書きは保持しています。差分を確認し、必要な内容を下書きへ取り込んでから再適用してください。
            </p>
            <button
              className={button}
              disabled={busy}
              onClick={() => setEditing(note)}
            >
              現在版を確認したので本文の下書きを再適用
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
          {viewed ? '過去版' : '現在版'} · revision {shown.revision} ·{' '}
          {date(shown.updated_at)} · 編集者: {shown.editor}
        </p>
        {shown.change_reason && <p>変更理由: {shown.change_reason}</p>}
        <div className="flex flex-wrap gap-base">
          <button
            className={button}
            disabled={busy}
            onClick={() => {
              setViewed(null);
              setEditing(note);
            }}
          >
            本文を編集
          </button>
          <button
            className={button}
            onClick={() => setHistoryOpen(!historyOpen)}
          >
            履歴・初版
          </button>
          {viewed && (
            <>
              <button className={button} onClick={() => setViewed(null)}>
                現在版へ
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
                この版を新しい版として復元
              </button>
            </>
          )}
        </div>
        {historyOpen && (
          <div className="space-y-base rounded-sm border border-border p-base">
            <p className="text-xs text-low">
              初版はこのメモの最初の記録です。Project創始時の思想とは限りません。
            </p>
            {history.error && <p role="alert">{String(history.error)}</p>}
            <div className="flex flex-wrap gap-base">
              {history.data?.map((n) => (
                <button
                  key={n.revision}
                  className={button}
                  onClick={() => setViewed(n)}
                >
                  {n.revision === 1 ? '初版' : `revision ${n.revision}`} ·{' '}
                  {date(n.updated_at)}
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
        <h3 className="text-lg text-high">会話からの追記</h3>
        <p className="text-xs text-low">
          本文とは別の記録です。出典の明確な最新の人間の判断を尊重し、AIの提案と区別します。
        </p>
        <div className="flex flex-wrap gap-base">
          <button
            className={button}
            disabled={busy}
            onClick={() => setAdding(!adding)}
          >
            追記する
          </button>
          <label>
            <input
              type="checkbox"
              checked={showHidden}
              onChange={(e) => setShowHidden(e.target.checked)}
            />{' '}
            非表示の追記も表示
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
  return (
    <>
      <h4 className="text-high break-words">
        {a.heading}
        {a.hidden && '（非表示）'}
      </h4>
      <p className="text-sm">{kinds[a.kind] ?? a.kind}</p>
      <Markdown value={a.content} />
      <dl className="space-y-half text-xs text-low break-all">
        <dt>発言者・考えの出所</dt>
        <dd>{a.speaker}</dd>
        <dt>出典</dt>
        <dd>{a.source}</dd>
        <dt>記録者</dt>
        <dd>{a.recorder}</dd>
        <dt>参照本文版</dt>
        <dd>revision {a.referenced_body_revision}</dd>
        <dt>記録日時 / 更新日時・編集者</dt>
        <dd>
          {date(a.created_at)} / {date(a.updated_at)} · {a.editor} ·
          追記revision {a.revision}
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
          追記を訂正
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
          {a.hidden ? '再表示' : '非表示にする'}
        </button>
        <button className={button} onClick={() => setHistoryOpen(!historyOpen)}>
          追記の履歴
        </button>
        <button
          className={button}
          disabled={busy || a.hidden || !!editing}
          onClick={() => {
            if (
              window.confirm(
                'この追記を現在の本文末尾に追加し、新しい本文revisionとして保存します。元の追記と履歴は残ります。'
              )
            )
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
          本文へ統合
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
          <p>
            保存済みの現在版は上に表示しています（追記revision {a.revision}
            ）。下書きは保持しています。出典と内容の差分を確認してから再適用してください。
          </p>
          <button
            className={button}
            disabled={busy}
            onClick={() => setEditing(a)}
          >
            現在版を確認したので追記の下書きを再適用
          </button>
        </div>
      )}
      {historyOpen && (
        <div className="space-y-base">
          {history.error && <p role="alert">{String(history.error)}</p>}
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
              heading: '短い見出し',
              speaker: '発言者',
              source: '出典（会話・発言の参照）',
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
        区分
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
      <div data-testid="addition-editor" className="border border-border">
        <WYSIWYGEditor
          value={value.content}
          onChange={(content) => setValue({ ...value, content })}
          disabled={busy}
          placeholder="重要な判断・提案・未決事項を短く記録"
          className="min-h-24 p-base"
        />
      </div>
      <label className="block">
        参照本文revision
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
        短い引用（任意）
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
          追記を保存
        </button>
        <button
          type="button"
          className={button}
          disabled={busy}
          onClick={onCancel}
        >
          キャンセル
        </button>
      </div>
    </form>
  );
}
