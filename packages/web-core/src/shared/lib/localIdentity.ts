export const LOCAL_USER_ID = '00000000-0000-0000-0000-000000000001';
export const LOCAL_ORG_ID = '00000000-0000-0000-0000-000000000002';

// Set from the server access gate before local providers mount. Never used as
// authentication evidence by the backend. The constant remains historical identity.
let serverUserId = LOCAL_USER_ID;
export function setLocalServerUserId(id: string) {
  serverUserId = id;
}
export function currentLocalUserId() {
  return serverUserId;
}
