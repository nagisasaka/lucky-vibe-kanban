import { stripAnsi } from 'fancy-ansi';

/** Keep the server's address: a remote localhost is not the browser's localhost. */
export function externalDevServerUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return ['http:', 'https:'].includes(url.protocol) &&
      !url.username &&
      !url.password
      ? url.toString()
      : null;
  } catch {
    return null;
  }
}

export function detectDevServerUrl(
  logs: Array<{ content: string }>
): string | null {
  let tail = '';
  for (
    let index = logs.length - 1;
    index >= 0 && tail.length < 16384;
    index -= 1
  ) {
    tail = logs[index].content.slice(-16384) + tail;
  }
  const text = stripAnsi(tail.slice(-16384));
  const matches = [...text.matchAll(/https?:\/\/[^\s<>"'`]+/g)];
  for (const match of matches.reverse()) {
    const url = externalDevServerUrl(match[0].replace(/[),;]+$/, ''));
    if (url) return url;
  }
  return null;
}
