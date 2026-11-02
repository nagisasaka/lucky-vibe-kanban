/** Original diagnostic text for React text content, never HTML or a translation key. */
export function errorDiagnostic(error: unknown): string {
  const parts: string[] = [];
  if (error && typeof error === 'object') {
    if ('message' in error && typeof error.message === 'string')
      parts.push(error.message);
    const detail = 'error_data' in error ? error.error_data : undefined;
    if (detail && typeof detail === 'object') {
      for (const key of [
        'message',
        'code',
        'type',
        'setting_key',
        'file_id',
        'recovery',
      ]) {
        const value = (detail as Record<string, unknown>)[key];
        if (typeof value === 'string' && value) parts.push(value);
      }
    }
  }
  return (
    [...new Set(parts)].join(' — ') || (error == null ? '' : String(error))
  );
}
