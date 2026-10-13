/** The trusted HTTPS proxy injects this only into a prefixed app document.
 * API and static assets retain the existing same-origin root routes.
 * Never derive the mount point from a tool argument or an arbitrary URL segment.
 */
export function publicRouterBasepath(): string {
  const prefix = document
    .querySelector('meta[name="lvk-basepath"]')
    ?.getAttribute('content');
  if (!prefix) return '/';
  if (!/^(\/[A-Za-z0-9_-]+)+$/.test(prefix)) {
    throw new Error('Invalid public application base path');
  }
  return prefix;
}
