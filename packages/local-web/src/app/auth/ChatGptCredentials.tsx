import {
  AccessRequestError,
  accessErrorNotice,
  toAccessRequestError,
} from '@/shared/lib/accessErrorMessage';
import { useTranslation } from 'react-i18next';
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
  onDirtyChange,
}: {
  onDirtyChange?: (dirty: boolean) => void;
  issuer: string;
  webClientId: string;
  disabled: boolean;
}) {
  const { t } = useTranslation('settings');
  const [saved, setSaved] = useState<Saved>();
  const [secret, setSecret] = useState('');
  const [visible, setVisible] = useState(false);
  const [clientId, setClientId] = useState('');
  const [inputSecret, setInputSecret] = useState('');
  const [savedClientIdDraft, setSavedClientIdDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AccessRequestError | null>(null);
  const [notice, setNotice] = useState<
    'saved' | 'secretCopied' | 'clientIdCopied' | null
  >(null);
  useEffect(() => {
    onDirtyChange?.(Boolean(clientId !== savedClientIdDraft || inputSecret));
    return () => onDirtyChange?.(false);
  }, [savedClientIdDraft, clientId, inputSecret, onDirtyChange]);
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
        if (active)
          setError(toAccessRequestError(e, 'auth.credentials.failed'));
      });
    return () => {
      active = false;
    };
  }, [ready, issuer, webClientId]);
  async function run(action: 'save' | 'copy' | 'show') {
    setBusy(true);
    setError(null);
    setNotice(null);
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
        setSavedClientIdDraft(clientId);
        setInputSecret('');
        setSecret('');
        setVisible(false);
        setNotice('saved');
      } else {
        const value = result.client_secret ?? '';
        if (!value)
          throw new AccessRequestError('auth.credentials.secretUnavailable');
        setSecret(value);
        if (action === 'copy') {
          await navigator.clipboard.writeText(value).catch((error) => {
            throw toAccessRequestError(
              error,
              'auth.credentials.copySecretFailed'
            );
          });
          setNotice('secretCopied');
        } else {
          setVisible(true);
        }
      }
    } catch (e) {
      setError(toAccessRequestError(e, 'auth.credentials.failed'));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section
      className="min-w-0 space-y-base rounded border p-base"
      aria-label={t('auth.credentials.title')}
    >
      <p>{t('auth.credentials.description')}</p>
      {error && (
        <p role="alert" className="text-error">
          {accessErrorNotice(error)}
        </p>
      )}
      {notice && <p role="status">{t(`auth.credentials.${notice}`)}</p>}
      <fieldset
        disabled={busy || disabled || !ready}
        className="min-w-0 space-y-base"
      >
        {saved?.available ? (
          <>
            <label className="block">
              {t('auth.credentials.clientId')}
              <input className={field} readOnly value={saved.client_id ?? ''} />
            </label>
            <button
              className={button}
              type="button"
              onClick={() =>
                void navigator.clipboard
                  .writeText(saved.client_id ?? '')
                  .then(() => setNotice('clientIdCopied'))
                  .catch(() =>
                    setError(
                      new AccessRequestError('auth.credentials.copyFailed')
                    )
                  )
              }
            >
              {t('auth.credentials.copyClientId')}
            </button>
            <label className="block">
              {t('auth.credentials.clientSecret')}
              <input
                className={field}
                readOnly
                type={visible ? 'text' : 'password'}
                autoComplete="off"
                value={secret}
                placeholder={t('auth.credentials.stored')}
              />
            </label>
            <div className="flex flex-wrap gap-base">
              <button
                className={button}
                type="button"
                onClick={() => void run('copy')}
              >
                {t('auth.credentials.copySecret')}
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
                {visible
                  ? t('auth.credentials.hideSecret')
                  : t('auth.credentials.showSecret')}
              </button>
            </div>
          </>
        ) : (
          <p>{t('auth.credentials.empty')}</p>
        )}
        <details>
          <summary className="cursor-pointer">
            {t('auth.credentials.update')}
          </summary>
          <div className="space-y-base pt-base">
            <p>{t('auth.credentials.updateHint')}</p>
            <label className="block">
              {t('auth.credentials.newClientId')}
              <input
                className={field}
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                autoComplete="off"
              />
            </label>
            <label className="block">
              {t('auth.credentials.newClientSecret')}
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
              {t('auth.credentials.save')}
            </button>
          </div>
        </details>
      </fieldset>
    </section>
  );
}
