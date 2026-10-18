import { useEffect, useState } from 'react';
import { accessRequest } from './serverAccess';

interface Saved {
  available: boolean;
  client_id?: string;
  client_secret?: string;
}
const field = 'min-w-0 w-full rounded border bg-secondary p-base text-normal';
const button =
  'rounded border bg-secondary px-double py-base text-high hover:bg-panel disabled:opacity-50';

export function ChatGptCredentials({
  issuer,
  webClientId,
  disabled,
}: {
  issuer: string;
  webClientId: string;
  disabled: boolean;
}) {
  const [saved, setSaved] = useState<Saved>();
  const [secret, setSecret] = useState('');
  const [visible, setVisible] = useState(false);
  const [clientId, setClientId] = useState('');
  const [inputSecret, setInputSecret] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const ready = Boolean(issuer && webClientId);
  useEffect(() => {
    if (!ready) return;
    let active = true;
    void accessRequest<Saved>('auth0/chatgpt-credentials', {
      action: 'status',
      issuer,
      web_client_id: webClientId,
    })
      .then((value) => {
        if (active) setSaved(value);
      })
      .catch((e: Error) => {
        if (active) setError(e.message);
      });
    return () => {
      active = false;
    };
  }, [ready, issuer, webClientId]);
  async function run(action: 'save' | 'copy' | 'show') {
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const result = await accessRequest<Saved>('auth0/chatgpt-credentials', {
        action: action === 'save' ? 'save' : 'reveal',
        issuer,
        web_client_id: webClientId,
        ...(action === 'save'
          ? { client_id: clientId, client_secret: inputSecret }
          : {}),
      });
      setSaved({ available: result.available, client_id: result.client_id });
      if (action === 'save') {
        setInputSecret('');
        setSecret('');
        setVisible(false);
        setNotice(
          '接続情報を保存しました。ログイン後もここからコピーできます。'
        );
      } else {
        const value = result.client_secret ?? '';
        if (!value) throw new Error('保存されたSecretを取得できませんでした');
        setSecret(value);
        if (action === 'copy') {
          await navigator.clipboard.writeText(value);
          setNotice('ChatGPT用Secretをコピーしました');
        } else {
          setVisible(true);
        }
      }
    } catch (e) {
      setError(
        e instanceof Error ? e.message : '接続情報を取得できませんでした'
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <section
      className="min-w-0 space-y-base rounded border p-base"
      aria-label="保存したChatGPT接続情報"
    >
      <p>
        ChatGPT用の接続情報はサーバーに暗号化して保存します。Auth0ログインの有効化後も、管理者はこの画面からコピーできます。
      </p>
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      <fieldset
        disabled={busy || disabled || !ready}
        className="min-w-0 space-y-base"
      >
        {saved?.available ? (
          <>
            <label className="block">
              ChatGPT用Client ID
              <input className={field} readOnly value={saved.client_id ?? ''} />
            </label>
            <button
              className={button}
              type="button"
              onClick={() =>
                void navigator.clipboard
                  .writeText(saved.client_id ?? '')
                  .then(() => setNotice('Client IDをコピーしました'))
                  .catch(() =>
                    setError(
                      'コピーできませんでした。Client ID欄からコピーしてください。'
                    )
                  )
              }
            >
              ChatGPT用Client IDをコピー
            </button>
            <label className="block">
              ChatGPT用Client Secret
              <input
                className={field}
                readOnly
                type={visible ? 'text' : 'password'}
                autoComplete="off"
                value={secret}
                placeholder="保存済み"
              />
            </label>
            <div className="flex flex-wrap gap-base">
              <button
                className={button}
                type="button"
                onClick={() => void run('copy')}
              >
                ChatGPT用Secretをコピー
              </button>
              <button
                className={button}
                type="button"
                onClick={() => {
                  if (visible) {
                    setVisible(false);
                    setSecret('');
                  } else {
                    void run('show');
                  }
                }}
              >
                {visible ? 'Secretを隠す' : 'Secretを表示'}
              </button>
            </div>
          </>
        ) : (
          <p>
            保存された接続情報はありません。自動セットアップするか、既存のChatGPT用アプリの情報を保存してください。
          </p>
        )}
        <details>
          <summary className="cursor-pointer">
            既存の接続情報を保存・更新
          </summary>
          <div className="space-y-base pt-base">
            <p>
              Auth0で取得したChatGPT専用アプリの情報を入力します。Auth0側の設定は変更しません。SecretをAuth0で変更した場合もここで更新してください。
            </p>
            <label className="block">
              保存するChatGPT Client ID
              <input
                className={field}
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                autoComplete="off"
              />
            </label>
            <label className="block">
              保存するChatGPT Client Secret
              <input
                className={field}
                type="password"
                value={inputSecret}
                onChange={(e) => setInputSecret(e.target.value)}
                autoComplete="off"
              />
            </label>
            <button
              className={button}
              type="button"
              disabled={!clientId || !inputSecret}
              onClick={() => void run('save')}
            >
              ChatGPT接続情報を保存
            </button>
          </div>
        </details>
      </fieldset>
    </section>
  );
}
