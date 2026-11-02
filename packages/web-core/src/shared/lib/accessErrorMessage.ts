import i18n from '@/i18n/config';

// Only known LVK messages become UI text. Provider bodies and unknown server
// errors must not expose secrets or internal details on the authentication screen.
const messages = {
  'Account is not active': 'auth.errors.notActive',
  'Administrator required': 'auth.errors.operatorRequired',
  'Another Auth0 setup is running; retry after it finishes':
    'auth.errors.setupBusy',
  'Application belongs to another LVK URL or purpose; select a different application':
    'auth.errors.otherApplication',
  'Auth0 Management API access token exceeds the 65536-byte limit':
    'auth.errors.tokenTooLong',
  'Auth0 did not return the client secret; check read:client_keys permission':
    'auth.errors.secretPermission',
  'Auth0 discovery does not advertise PKCE S256; check tenant settings':
    'auth.errors.pkceRequired',
  'Auth0 discovery must advertise client_secret_basic for the configured clients':
    'auth.errors.clientSecretBasicRequired',
  'Auth0 login is disabled': 'auth.errors.auth0Disabled',
  'Auth0 required': 'auth.errors.auth0Required',
  'Auth0 setup exceeded the overall 120-second timeout; preview again before retrying':
    'auth.errors.setupTimeout',
  'Auth0 tenant has too many entries for automatic setup':
    'auth.errors.tooManyEntries',
  'Authentication busy': 'auth.errors.authenticationBusy',
  'Authentication required': 'auth.errors.authenticationRequired',
  'Authentication unavailable; operator recovery may be required':
    'auth.errors.authenticationUnavailable',
  'Basic login is disabled': 'auth.errors.basicDisabled',
  'CSRF check failed': 'auth.errors.csrfFailed',
  'Cannot disable or demote the last active administrator':
    'auth.errors.lastOperator',
  'Configuration changed; restart setup': 'auth.errors.configurationChanged',
  'Configure a public HTTPS URL before Auth0 setup':
    'auth.errors.publicUrlRequired',
  'Configure a public HTTPS URL before saving ChatGPT credentials':
    'auth.errors.credentialsPublicUrl',
  'Enter a dedicated ChatGPT Client ID and Client Secret, separate from the Web application':
    'auth.errors.dedicatedCredentials',
  'Enter an Auth0 Management API access token': 'auth.errors.tokenRequired',
  'Enter the exact HTTPS callback URL shown by ChatGPT':
    'auth.errors.callbackRequired',
  'Existing MCP API is not RS256; review it in Auth0 before continuing':
    'auth.errors.apiAlgorithm',
  'Incomplete Auth0 response': 'auth.errors.incompleteResponse',
  'Invalid Auth0 issuer': 'auth.errors.invalidIssuer',
  'Invalid Auth0 object ID': 'auth.errors.invalidObject',
  'Invalid Basic username': 'auth.errors.invalidUsername',
  'Invalid host': 'auth.errors.invalidHost',
  'Invalid member state or role': 'auth.errors.invalidMember',
  'Invalid origin': 'auth.errors.invalidOrigin',
  'Local deployment only accepts loopback hosts': 'auth.errors.loopbackOnly',
  'Member belongs to an inactive authentication provider':
    'auth.errors.inactiveProvider',
  'Membership approval required or account disabled':
    'auth.errors.approvalRequired',
  'No saved ChatGPT credentials match this Auth0 issuer and Web application':
    'auth.errors.credentialsMismatch',
  'Only Regular Web Applications can be reused': 'auth.errors.regularWebApp',
  'Password must be 12 to 1024 bytes': 'auth.errors.passwordLength',
  'Preview the Auth0 changes before applying': 'auth.errors.previewRequired',
  'Public HTTPS URL, Web Client ID and Web Client Secret are required':
    'auth.errors.webCredentialsRequired',
  'Remote MCP is disabled': 'auth.errors.mcpDisabled',
  'Restart with the public HTTPS deployment preset first':
    'auth.errors.publicPreset',
  'Select an existing GitHub or Google social connection':
    'auth.errors.connectionRequired',
  'Select exactly one existing application or create a new one':
    'auth.errors.applicationRequired',
  'Server setup is incomplete': 'auth.errors.setupIncomplete',
  'Settings changed or preview expired; preview the Auth0 changes again':
    'auth.errors.previewExpired',
  'Setup already completed': 'auth.errors.alreadyConfigured',
  'This MCP transport is stateless; omit MCP Session ID':
    'auth.errors.statelessMcp',
  'This mode is disabled by deployment configuration':
    'auth.errors.modeDisabled',
  'Too many authentication attempts; retry in one minute':
    'auth.errors.rateLimit',
  'Unexpected Auth0 list response': 'auth.errors.unexpectedResponse',
  'Unknown member': 'auth.errors.unknownMember',
  'Use the exact Auth0 tenant issuer: https://TENANT.REGION.auth0.com/ (including the final slash)':
    'auth.errors.issuerFormat',
  'Web and ChatGPT must use separate applications': 'auth.errors.separateApps',
  insufficient_scope: 'auth.errors.insufficientScope',
} as const;

type AccessNoticeKey =
  | (typeof messages)[keyof typeof messages]
  | 'auth.errors.forbidden'
  | 'auth.errors.requestFailed'
  | 'auth.errors.networkFailed'
  | 'auth.errors.statusFailed'
  | 'auth.errors.signInRequired'
  | 'auth.gate.statusFailed'
  | 'auth.gate.actionFailed'
  | 'auth.setup.failed'
  | 'auth.credentials.failed'
  | 'auth.credentials.secretUnavailable'
  | 'auth.credentials.copySecretFailed'
  | 'auth.credentials.copyFailed';

function accessErrorKey(message: unknown, status: number): AccessNoticeKey {
  const key =
    typeof message === 'string' && Object.hasOwn(messages, message)
      ? messages[message as keyof typeof messages]
      : undefined;
  if (key) return key;
  if (status === 401) return 'auth.errors.authenticationRequired';
  if (status === 403) return 'auth.errors.forbidden';
  if (status === 429) return 'auth.errors.rateLimit';
  return 'auth.errors.requestFailed';
}

// Keep only a trusted reason, never the provider body or a translated snapshot.
export class AccessRequestError extends Error {
  constructor(readonly notice: AccessNoticeKey) {
    super(notice);
  }
}

export function toAccessRequestError(
  error: unknown,
  fallback: AccessNoticeKey
): AccessRequestError {
  return error instanceof AccessRequestError
    ? error
    : new AccessRequestError(fallback);
}

export function accessRequestFailure(message: unknown, status: number) {
  return new AccessRequestError(accessErrorKey(message, status));
}

export function accessErrorNotice(error: AccessRequestError): string {
  return i18n.t(error.notice, { ns: 'settings' });
}

export function accessErrorMessage(message: unknown, status: number): string {
  return i18n.t(accessErrorKey(message, status), { ns: 'settings' });
}
