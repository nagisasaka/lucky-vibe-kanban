export function formatFileSize(
  bytes: bigint | number | null | undefined,
  locale?: string
): string {
  if (!bytes) return '';
  if (locale) {
    const count =
      typeof bytes === 'bigint'
        ? bytes
        : Number.isFinite(bytes)
          ? BigInt(Math.trunc(bytes))
          : null;
    if (count === null) return '';
    const formatter = new Intl.NumberFormat(locale);
    if (count < 1024n) return `${formatter.format(count)} B`;
    const divisor = count < 1048576n ? 1024n : 1048576n;
    const unit = divisor === 1024n ? 'KB' : 'MB';
    // Round to one decimal without coercing large byte counts to Number.
    const tenths = (count * 10n + divisor / 2n) / divisor;
    const decimal =
      formatter.formatToParts(1.1).find((part) => part.type === 'decimal')
        ?.value ?? '.';
    const digit = new Intl.NumberFormat(locale, { useGrouping: false }).format(
      tenths % 10n
    );
    return `${formatter.format(tenths / 10n)}${decimal}${digit} ${unit}`;
  }
  const num = Number(bytes);
  if (num < 1024) return `${num} B`;
  if (num < 1024 * 1024) return `${(num / 1024).toFixed(1)} KB`;
  return `${(num / (1024 * 1024)).toFixed(1)} MB`;
}
