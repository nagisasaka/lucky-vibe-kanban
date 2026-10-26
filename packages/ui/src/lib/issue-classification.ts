/** Reserved classification label; never an authorization or execution signal. */
export function isSystemTag(tag: { name: string }): boolean {
  return tag.name.trim().toLowerCase() === 'system';
}
