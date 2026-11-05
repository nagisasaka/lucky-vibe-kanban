import {
  AccessRequestError,
  accessErrorNotice,
  toAccessRequestError,
} from '@/shared/lib/accessErrorMessage';
import { SettingsAccessContext } from '@/shared/dialogs/settings/settings/SettingsAccessContext';
import { useSettingsDirty } from '@/shared/dialogs/settings/settings/SettingsDirtyContext';
import { useTranslation } from 'react-i18next';
import { rememberPushReturn, takePushReturn } from './pushReturn';
import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { Auth0Setup } from './Auth0Setup';
import { ChatGptCredentials } from './ChatGptCredentials';
import {
  accessRequest,
  loadAccessStatus,
  type AccessStatus,
} from './serverAccess';

interface Member {
  id: string;
  issuer: string;
  subject: string;
  display_name: string;
  state: string;
  role: string;
}
interface Configuration {
  public_url: string;
  callback_url: string;
  logout_url: string;
  resource: string;
  scopes: string[];
  current: {
    mode?: string;
    issuer?: string;
    client_id?: string;
    username?: string;
    mcp_enabled?: boolean;
    secret_configured?: boolean;
  };
}
interface Confirmation {
  ready: boolean;
  identity?: Member;
}
const field =
  'mt-2 min-h-11 min-w-0 w-full rounded-sm border border-border bg-secondary px-3 py-2 text-high outline-none focus-visible:ring-2 focus-visible:ring-brand disabled:opacity-50';
const buttonBase =
  'min-h-11 shrink-0 max-w-full whitespace-normal break-words rounded-sm border border-border px-4 py-2 font-medium focus-visible:ring-2 focus-visible:ring-brand disabled:cursor-not-allowed disabled:opacity-50';
const button = `${buttonBase} bg-secondary text-high hover:bg-panel`;
const primaryButton = `${buttonBase} w-full bg-brand text-on-brand hover:bg-brand-hover`;

function CopyValue({ label, value }: { label: string; value: string }) {
  const { t } = useTranslation('settings');
  const [copyStatus, setCopyStatus] = useState<'copied' | 'failed' | null>(
    null
  );
  return (
    <div className="space-y-half">
      <label className="text-low">{label}</label>
      <div className="flex gap-base">
        <input className={field} readOnly value={value} aria-label={label} />
        <button
          className={button}
          type="button"
          onClick={() =>
            void navigator.clipboard
              .writeText(value)
              .then(() => setCopyStatus('copied'))
              .catch(() => setCopyStatus('failed'))
          }
        >
          {t('auth.gate.copy')}
        </button>
      </div>
      {copyStatus && (
        <p role={copyStatus === 'failed' ? 'alert' : 'status'}>
          {copyStatus === 'copied'
            ? t('auth.gate.copied')
            : t('auth.gate.copyFailed')}
        </p>
      )}
    </div>
  );
}

function EmbeddedAccessSettings() {
  const { setDirty } = useSettingsDirty();
  const onDirtyChange = useCallback(
    (dirty: boolean) => setDirty('access', dirty),
    [setDirty]
  );
  useEffect(() => () => setDirty('access', false), [setDirty]);
  return (
    <ServerAccessGate embedded onDirtyChange={onDirtyChange}>
      {null}
    </ServerAccessGate>
  );
}

export function ServerAccessGate({
  children,
  embedded = false,
  onDirtyChange,
}: {
  children: ReactNode;
  embedded?: boolean;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { t, i18n } = useTranslation('settings');
  const [status, setStatus] = useState<AccessStatus>();
  const [setupDirty, setSetupDirty] = useState(false);
  const [credentialsDirty, setCredentialsDirty] = useState(false);
  const [error, setError] = useState<AccessRequestError | null>(null);
  const [busy, setBusy] = useState(false);
  const [provisioningBusy, setProvisioningBusy] = useState(false);
  const [key, setKey] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [issuer, setIssuer] = useState('');
  const [clientId, setClientId] = useState('');
  const [clientSecret, setClientSecret] = useState('');
  const [mcp, setMcp] = useState(false);
  const [credentialsVersion, setCredentialsVersion] = useState(0);
  const [mode, setMode] = useState('auth0');
  const [configuration, setConfiguration] = useState<Configuration>();
  const [confirmation, setConfirmation] = useState<Confirmation>();
  const [members, setMembers] = useState<Member[]>([]);
  const [audit, setAudit] = useState<
    Array<{
      id: number;
      at: number;
      actor: string | null;
      action: string;
      outcome: string;
    }>
  >([]);
  const view = new URLSearchParams(window.location.search).get('access');
  const settings =
    embedded || status?.limited || view === 'settings' || view === 'confirm';
  const recovery = view === 'recovery';
  const refresh = useCallback(async () => {
    try {
      setStatus(await loadAccessStatus());
    } catch (e) {
      setError(toAccessRequestError(e, 'auth.gate.statusFailed'));
    }
  }, []);
  useEffect(() => {
    void refresh();
    window.addEventListener('lvk-access-expired', refresh);
    return () => window.removeEventListener('lvk-access-expired', refresh);
  }, [refresh]);
  useEffect(() => {
    if (!status?.authenticated || settings || recovery) return;
    try {
      const target = takePushReturn(window.sessionStorage);
      if (target && window.location.pathname === '/')
        window.location.replace(target);
    } catch {
      /* Storage can be disabled; reopening the notification still works. */
    }
  }, [status?.authenticated, settings, recovery]);
  useEffect(() => {
    if (!settings || !status?.management) return;
    void Promise.all([
      accessRequest<Configuration>('configuration'),
      accessRequest<Confirmation>('confirmation'),
    ])
      .then(([config, confirmation]) => {
        setConfiguration(config);
        setConfirmation(confirmation);
        setIssuer(config.current.issuer ?? '');
        setClientId(config.current.client_id ?? '');
        setMcp(config.current.mcp_enabled ?? false);
        setUsername(config.current.username ?? '');
        setMode(
          status.forced_mode ??
            (config.current.mode === 'basic' ? 'basic' : 'auth0')
        );
      })
      .catch((e: Error) =>
        setError(toAccessRequestError(e, 'auth.gate.actionFailed'))
      );
    if (status.authenticated && status.mode !== 'local') {
      void accessRequest<{ members: Member[] }>('members')
        .then((r) => setMembers(r.members))
        .catch((e: Error) =>
          setError(toAccessRequestError(e, 'auth.gate.actionFailed'))
        );
      void accessRequest<{ events: typeof audit }>('audit')
        .then((r) => setAudit(r.events))
        .catch((e: Error) =>
          setError(toAccessRequestError(e, 'auth.gate.actionFailed'))
        );
    }
  }, [settings, status]);
  useEffect(() => {
    const current = configuration?.current;
    const initialMode =
      status?.forced_mode ?? (current?.mode === 'basic' ? 'basic' : 'auth0');
    onDirtyChange?.(
      Boolean(
        key ||
          password ||
          clientSecret ||
          setupDirty ||
          credentialsDirty ||
          (current &&
            (username !== (current.username ?? '') ||
              issuer !== (current.issuer ?? '') ||
              clientId !== (current.client_id ?? '') ||
              mcp !== (current.mcp_enabled ?? false) ||
              mode !== initialMode))
      )
    );
  }, [
    configuration,
    status?.forced_mode,
    key,
    password,
    clientSecret,
    setupDirty,
    credentialsDirty,
    username,
    issuer,
    clientId,
    mcp,
    mode,
    onDirtyChange,
  ]);
  function navigateAfterSuccess(target: string) {
    // Authentication transitions have already persisted or staged their input.
    // Clear this screen's leave guard before the intentional full-page handoff.
    onDirtyChange?.(false);
    window.location.assign(target);
  }
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(toAccessRequestError(e, 'auth.gate.actionFailed'));
    } finally {
      setBusy(false);
    }
  }
  async function login() {
    await run(async () => {
      try {
        rememberPushReturn(
          window.sessionStorage,
          window.location.pathname + window.location.search
        );
      } catch {
        /* A blocked storage getter must not prevent login. */
      }
      const result = await accessRequest<{ authorize_url: string }>('login');
      navigateAfterSuccess(result.authorize_url);
    });
  }
  if (status?.authenticated && !settings && !recovery) {
    return (
      <SettingsAccessContext.Provider
        value={{
          management: status.user?.admin ? <EmbeddedAccessSettings /> : null,
          account: (
            <section
              className="space-y-double p-double"
              aria-label={t('settings.account.session')}
            >
              <h3 className="text-base font-medium text-high">
                {t('settings.account.session')}
              </h3>
              <dl className="space-y-base">
                <div className="min-w-0">
                  <dt className="text-low">
                    {t(
                      status.mode === 'local'
                        ? 'settings.account.connection'
                        : status.mode === 'basic'
                          ? 'settings.account.username'
                          : 'settings.account.identity'
                    )}
                  </dt>
                  <dd className="break-words [overflow-wrap:anywhere]">
                    {status.mode === 'local'
                      ? t('settings.account.localStatus')
                      : status.user?.display_name?.trim() ||
                        t('settings.account.unavailable')}
                  </dd>
                </div>
                <div>
                  <dt className="text-low">{t('settings.account.method')}</dt>
                  <dd>
                    {status.mode === 'auth0'
                      ? 'Auth0'
                      : status.mode === 'basic'
                        ? 'Basic'
                        : t('settings.account.localMethod')}
                  </dd>
                </div>
              </dl>
              <p className="text-low text-sm">
                {t(
                  status.mode === 'local'
                    ? 'settings.account.localHint'
                    : 'settings.account.logoutHint'
                )}
              </p>
              {error && (
                <p role="alert" className="text-error">
                  {accessErrorNotice(error)}
                </p>
              )}
              {status.mode !== 'local' && (
                <button
                  type="button"
                  className={button}
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      await accessRequest('logout', {});
                      navigateAfterSuccess('/');
                    })
                  }
                >
                  {t('settings.account.logout')}
                </button>
              )}
            </section>
          ),
        }}
      >
        {children}
      </SettingsAccessContext.Provider>
    );
  }

  return (
    <main
      className={
        embedded
          ? 'new-design min-w-0 bg-primary p-base text-base text-normal'
          : 'new-design grid min-h-svh place-items-center overflow-auto bg-secondary px-4 py-8 text-sm leading-relaxed text-normal sm:px-6'
      }
    >
      <div
        className={
          embedded
            ? 'mx-auto max-w-2xl space-y-double p-double'
            : `mx-auto w-full space-y-6 rounded-lg border border-border bg-primary p-6 shadow-sm sm:p-8 ${settings ? 'max-w-3xl' : 'max-w-[480px]'}`
        }
      >
        {!embedded && (
          <div className="flex items-center gap-3 border-b border-border pb-6">
            <img
              src="/lvk-clover.svg"
              alt=""
              aria-hidden="true"
              className="size-10 shrink-0"
            />
            <span className="text-base font-semibold tracking-tight text-high">
              Lucky Vibe Kanban
            </span>
          </div>
        )}
        <h1 className="text-xl font-semibold leading-snug tracking-tight text-high">
          LVK{' '}
          {settings
            ? t('auth.gate.managementTitle')
            : t('auth.gate.loginTitle')}
        </h1>
        {error && (
          <p
            role="alert"
            className="rounded-sm border border-error/30 bg-error/10 px-3 py-3 text-error"
          >
            {accessErrorNotice(error)}
          </p>
        )}
        {!status && (
          <p className="rounded-sm border border-border bg-secondary px-3 py-3 text-low">
            {t('auth.gate.checking')}
          </p>
        )}
        {status?.configuration_error && (
          <p className="rounded-sm border border-error/30 bg-error/10 px-3 py-3 text-error">
            {t('auth.gate.configurationError')}
          </p>
        )}
        {status?.mode === 'locked' && !status.limited && (
          <p className="rounded-sm border border-border bg-secondary px-3 py-3">
            {t('auth.gate.locked')}
          </p>
        )}
        {(recovery ||
          (status &&
            !status.authenticated &&
            !status.limited &&
            status.mode === 'locked') ||
          (settings && status?.mode === 'local' && !configuration)) && (
          <form
            className="space-y-base"
            onSubmit={(event) => {
              event.preventDefault();
              void run(async () => {
                await accessRequest(
                  'setup',
                  { key },
                  { 'x-lvk-csrf': 'setup' }
                );
                setKey('');
                navigateAfterSuccess('/?access=settings');
              });
            }}
          >
            <p>{t('auth.gate.keyCommand')}</p>
            <code className="block overflow-auto bg-secondary p-base">
              docker compose exec lvk server{' '}
              {recovery ? 'recovery-token' : 'setup-token'}
            </code>
            <label className="block">
              {recovery ? t('auth.gate.recoveryKey') : t('auth.gate.setupKey')}
              <input
                className={field}
                type="password"
                autoComplete="off"
                required
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
            </label>
            <button
              className={primaryButton}
              disabled={busy || provisioningBusy}
            >
              {t('auth.gate.startSetup')}
            </button>
            <p className="text-low">{t('auth.gate.setupSession')}</p>
          </form>
        )}
        {status?.mode === 'basic' &&
          !status.authenticated &&
          !recovery &&
          !status.limited && (
            <form
              className="space-y-base"
              onSubmit={(event) => {
                event.preventDefault();
                void run(async () => {
                  const encoded = btoa(
                    Array.from(
                      new TextEncoder().encode(`${username}:${password}`),
                      (c) => String.fromCharCode(c)
                    ).join('')
                  );
                  await accessRequest(
                    'basic-login',
                    {},
                    { authorization: `Basic ${encoded}`, 'x-lvk-csrf': 'basic' }
                  );
                  setPassword('');
                  await refresh();
                });
              }}
            >
              <label className="block">
                {t('auth.gate.username')}
                <input
                  className={field}
                  autoComplete="username"
                  required
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                />
              </label>
              <label className="block">
                {t('auth.gate.password')}
                <input
                  className={field}
                  type="password"
                  autoComplete="current-password"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </label>
              <button
                className={primaryButton}
                disabled={busy || provisioningBusy}
              >
                {t('auth.gate.signIn')}
              </button>
            </form>
          )}
        {status?.mode === 'auth0' &&
          !status.authenticated &&
          !recovery &&
          !status.limited && (
            <div className="space-y-base">
              {view === 'pending' && (
                <p className="rounded-sm border border-border bg-secondary px-3 py-3">
                  {t('auth.gate.pending')}
                </p>
              )}
              <button
                className={primaryButton}
                disabled={busy || provisioningBusy}
                onClick={() => void login()}
              >
                {t('auth.gate.signInAuth0')}
              </button>
              <p className="text-sm leading-relaxed text-low">
                {t('auth.gate.providerHint')}
              </p>
            </div>
          )}
        {settings && configuration && status?.management && (
          <>
            <CopyValue
              label={t('auth.gate.publicUrl')}
              value={configuration.public_url || 'Local'}
            />
            <label className="block">
              {t('auth.gate.method')}
              <select
                aria-label={t('auth.gate.method')}
                className={field}
                value={mode}
                disabled={
                  Boolean(status.forced_mode) || busy || provisioningBusy
                }
                onChange={(e) => setMode(e.target.value)}
              >
                {status.deployment === 'local' && (
                  <option value="local">{t('auth.gate.localOption')}</option>
                )}
                <option value="basic">{t('auth.gate.basicOption')}</option>
                <option value="auth0">{t('auth.gate.auth0Option')}</option>
              </select>
            </label>
            {status.forced_mode && <p>{t('auth.gate.forcedMode')}</p>}
            {mode === 'local' && (
              <button
                className={button}
                disabled={busy || provisioningBusy}
                onClick={() =>
                  void run(async () => {
                    await accessRequest('local', {});
                    navigateAfterSuccess('/');
                  })
                }
              >
                {t('auth.gate.enableLocal')}
              </button>
            )}
            {mode === 'basic' && (
              <form
                className="space-y-base"
                onSubmit={(event) => {
                  event.preventDefault();
                  void run(async () => {
                    await accessRequest('basic', { username, password });
                    setPassword('');
                    navigateAfterSuccess('/');
                  });
                }}
              >
                <p>{t('auth.gate.basicHint')}</p>
                <label className="block">
                  {t('auth.gate.username')}
                  <input
                    className={field}
                    required
                    value={username}
                    autoComplete="username"
                    onChange={(e) => setUsername(e.target.value)}
                  />
                </label>
                <label className="block">
                  {t('auth.gate.newPassword')}
                  <input
                    className={field}
                    type="password"
                    autoComplete="new-password"
                    minLength={12}
                    required
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                  />
                </label>
                <button className={button} disabled={busy || provisioningBusy}>
                  {t('auth.gate.enableBasic')}
                </button>
              </form>
            )}
            {mode === 'auth0' && (
              <div className="space-y-double">
                <p>{t('auth.gate.auth0Hint')}</p>
                <Auth0Setup
                  onDirtyChange={setSetupDirty}
                  issuer={issuer}
                  currentClientId={clientId}
                  disabled={busy || provisioningBusy}
                  onBusyChange={setProvisioningBusy}
                  onIssuerChange={setIssuer}
                  onConfigured={(result) => {
                    setIssuer(result.issuer);
                    setClientId(result.web.client_id);
                    setClientSecret(result.web.client_secret);
                    setMcp(result.mcp_enabled);
                    setCredentialsVersion((version) => version + 1);
                    setConfirmation(undefined);
                  }}
                />
                <CopyValue
                  label={t('auth.gate.callbackUrl')}
                  value={configuration.callback_url}
                />
                <CopyValue
                  label={t('auth.gate.logoutUrl')}
                  value={configuration.logout_url}
                />
                <form
                  className="space-y-base"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void run(async () => {
                      const result = await accessRequest<{
                        authorize_url: string;
                      }>('auth0/test', {
                        issuer,
                        client_id: clientId,
                        client_secret: clientSecret,
                        mcp_enabled: mcp,
                      });
                      setClientSecret('');
                      navigateAfterSuccess(result.authorize_url);
                    });
                  }}
                >
                  <label className="block">
                    {t('auth.gate.issuer')}
                    <input
                      className={field}
                      type="url"
                      placeholder="https://your-tenant.jp.auth0.com/"
                      required
                      value={issuer}
                      onChange={(e) => setIssuer(e.target.value)}
                    />
                  </label>
                  <label className="block">
                    {t('auth.gate.webClientId')}
                    <input
                      className={field}
                      required
                      value={clientId}
                      onChange={(e) => setClientId(e.target.value)}
                    />
                  </label>
                  <label className="block">
                    {t('auth.gate.webClientSecret')}
                    <input
                      className={field}
                      type="password"
                      autoComplete="off"
                      required={!configuration.current.secret_configured}
                      placeholder={
                        configuration.current.secret_configured
                          ? t('auth.gate.keepSecret')
                          : ''
                      }
                      value={clientSecret}
                      onChange={(e) => setClientSecret(e.target.value)}
                    />
                  </label>
                  <label className="flex gap-base">
                    <input
                      type="checkbox"
                      checked={mcp}
                      onChange={(e) => setMcp(e.target.checked)}
                    />
                    {t('auth.gate.enableMcp')}
                  </label>
                  <p className="text-low">{t('auth.gate.testHint')}</p>
                  <button
                    className={button}
                    disabled={busy || provisioningBusy}
                  >
                    {t('auth.gate.testLogin')}
                  </button>
                </form>
                {confirmation?.ready && confirmation.identity && (
                  <div className="space-y-base rounded border p-base">
                    <h2 className="text-lg text-high">
                      {t('auth.gate.testSucceeded')}
                    </h2>
                    <p>{t('auth.gate.confirmIdentity')}</p>
                    <p>{confirmation.identity.display_name}</p>
                    <code className="block break-all">
                      {confirmation.identity.issuer} /{' '}
                      {confirmation.identity.subject}
                    </code>
                    <button
                      className={button}
                      disabled={busy || provisioningBusy}
                      onClick={() =>
                        void run(async () => {
                          await accessRequest('activate', {});
                          navigateAfterSuccess('/');
                        })
                      }
                    >
                      {t('auth.gate.activateAuth0')}
                    </button>
                  </div>
                )}
                <h2 className="text-lg text-high">
                  {t('auth.gate.chatgptTitle')}
                </h2>
                <ChatGptCredentials
                  onDirtyChange={setCredentialsDirty}
                  key={`${issuer}:${clientId}:${credentialsVersion}`}
                  issuer={issuer}
                  webClientId={clientId}
                  disabled={busy || provisioningBusy}
                />
                <p>{t('auth.gate.manualHint')}</p>
                <CopyValue
                  label="MCP URL / Auth0 API Identifier / Resource / Audience"
                  value={configuration.resource}
                />
                <CopyValue
                  label={t('auth.gate.permissions')}
                  value={configuration.scopes.join(' ')}
                />
                <ol className="list-decimal space-y-base pl-double">
                  <li>{t('auth.gate.manualApi')}</li>
                  <li>{t('auth.gate.manualApp')}</li>
                  <li>{t('auth.gate.manualCallback')}</li>
                  <li>{t('auth.gate.manualVerify')}</li>
                </ol>
                <p>{t('auth.gate.chatgptUnverified')}</p>
              </div>
            )}
            {status.authenticated && status.mode === 'auth0' && (
              <div className="space-y-base">
                <h2 className="text-lg text-high">
                  {t('auth.gate.membersTitle')}
                </h2>
                <p>{t('auth.gate.membersHint')}</p>
                {members.map((member) => (
                  <div
                    className="space-y-half rounded border p-base"
                    key={member.id}
                  >
                    <strong>{member.display_name}</strong>
                    <p className="break-all text-low">
                      {member.issuer} / {member.subject}
                    </p>
                    <p>
                      {member.state === 'active'
                        ? t('auth.gate.active')
                        : member.state === 'disabled'
                          ? t('auth.gate.disabled')
                          : member.state === 'pending'
                            ? t('auth.gate.pendingState')
                            : member.state}{' '}
                      ·{' '}
                      {member.role === 'admin'
                        ? t('auth.gate.operatorRole')
                        : member.role === 'member'
                          ? t('auth.gate.memberRole')
                          : member.role}
                    </p>
                    <div className="flex flex-wrap gap-base">
                      {(['active', 'disabled'] as const).map((state) => (
                        <button
                          key={state}
                          className={button}
                          disabled={busy || member.state === state}
                          onClick={() =>
                            void run(async () => {
                              await accessRequest('members', {
                                id: member.id,
                                state,
                                role: member.role,
                              });
                              const result = await accessRequest<{
                                members: Member[];
                              }>('members');
                              setMembers(result.members);
                            })
                          }
                        >
                          {state === 'active'
                            ? t('auth.gate.approveMember')
                            : t('auth.gate.disableMember')}
                        </button>
                      ))}
                      <button
                        className={button}
                        disabled={busy || member.state !== 'active'}
                        onClick={() =>
                          void run(async () => {
                            await accessRequest('members', {
                              id: member.id,
                              state: member.state,
                              role:
                                member.role === 'admin' ? 'member' : 'admin',
                            });
                            const result = await accessRequest<{
                              members: Member[];
                            }>('members');
                            setMembers(result.members);
                          })
                        }
                      >
                        {member.role === 'admin'
                          ? t('auth.gate.makeMember')
                          : t('auth.gate.makeOperator')}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
            {audit.length > 0 && (
              <details>
                <summary>{t('auth.gate.audit')}</summary>
                <ul className="space-y-half">
                  {audit.map((event) => (
                    <li key={event.id}>
                      {new Date(event.at * 1000).toLocaleString(
                        i18n.resolvedLanguage || i18n.language
                      )}{' '}
                      · {event.actor ?? t('auth.gate.anonymousActor')} ·{' '}
                      {event.action} · {event.outcome}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            <p className="text-low">{t('auth.gate.sessionHint')}</p>
            {status.authenticated && !embedded && (
              <a className="block underline" href="/">
                {t('auth.gate.back')}
              </a>
            )}
          </>
        )}
        {status && !status.limited && !recovery && (
          <a
            className="block border-t border-border pt-4 text-xs text-low underline underline-offset-4 hover:text-high focus-visible:ring-2 focus-visible:ring-brand"
            href="/?access=recovery"
          >
            {t('auth.gate.recover')}
          </a>
        )}
      </div>
    </main>
  );
}
