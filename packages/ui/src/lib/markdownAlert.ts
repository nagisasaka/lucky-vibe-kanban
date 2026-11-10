const ALERTS = {
  DECISION: ['decision', '◇'],
  NOTE: ['note', 'ⓘ'],
  TIP: ['tip', '✧'],
  IMPORTANT: ['important', '❖'],
  WARNING: ['warning', '⚠'],
  CAUTION: ['caution', '⛔'],
} as const;

/** Match only an explicit, standalone marker at the start of a quote/paragraph. */
export function getMarkdownAlert(text: string) {
  const match = /^\[!([A-Z]+)\](?:\r?\n|$)/.exec(text);
  if (!match || !Object.prototype.hasOwnProperty.call(ALERTS, match[1])) return;
  const [name, icon] = ALERTS[match[1] as keyof typeof ALERTS];
  return { marker: match[0], name, icon };
}
