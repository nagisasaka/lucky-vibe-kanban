/** Preview text is disposable; details and clipboard always use the source. */
function clippedText(source: string, limit: number) {
  return source.slice(0, limit).replace(/[\uD800-\uDBFF]$/, '');
}

export function toolCommandHeading(source: string) {
  const lines = source.split(/\r?\n/);
  const first = lines.find((line) => line.trim().length > 0) ?? '';
  return `${clippedText(first, 160)}${first.length > 160 || lines.length > 1 ? ' …' : ''}`;
}

export function toolOutputPreview(source: string, failed: boolean) {
  const lines = source.split('\n');
  if (lines.at(-1) === '') lines.pop();
  let indexes = lines.map((_, index) => index).slice(-3);
  if (failed && lines.length > 4) {
    const diagnostic = lines.findIndex((line) =>
      /\b(error|failed|failure|exception|panic|fatal)\b|エラー|失敗/i.test(line)
    );
    const start = diagnostic < 0 ? 0 : Math.max(0, diagnostic - 1);
    indexes = [
      ...new Set([
        start,
        start + 1,
        start + 2,
        lines.length - 2,
        lines.length - 1,
      ]),
    ]
      .filter((index) => index < lines.length)
      .sort((a, b) => a - b);
  } else if (failed) {
    indexes = lines.map((_, index) => index);
  }
  const selected: string[] = [];
  indexes.forEach((index, position) => {
    if (position > 0 && index > indexes[position - 1] + 1) selected.push('…');
    selected.push(lines[index]);
  });
  return {
    text: selected
      .map((line) => (line.length > 240 ? `${clippedText(line, 240)}…` : line))
      .join('\n'),
    clipped:
      indexes.length < lines.length ||
      selected.some((line) => line.length > 240),
  };
}
