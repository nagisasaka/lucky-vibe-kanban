export function normalizeProjectKey(value: string): string {
  // Unicode White_Space matches Rust str::trim (unlike JavaScript trim).
  return value
    .replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '')
    .replace(/[a-z]/g, (letter) => letter.toUpperCase());
}

export function isValidProjectKey(value: string): boolean {
  return /^[A-Z][A-Z0-9]{1,9}$/.test(normalizeProjectKey(value));
}

export function suggestProjectKey(
  name: string,
  existingKeys: string[]
): string {
  const words = name
    .trim()
    .split(/[\s_-]+/)
    .filter(Boolean);
  if (!words.length || words.some((word) => !/^[A-Za-z0-9]+$/.test(word))) {
    return '';
  }
  const base = (
    words.length > 1
      ? words.map((word) => word[0]).join('')
      : words[0].slice(0, 3)
  )
    .toUpperCase()
    .slice(0, 10);
  if ((base.match(/[A-Z]/g)?.length ?? 0) < 2 || !/^[A-Z]/.test(base))
    return '';
  const used = new Set(existingKeys.map(normalizeProjectKey));
  if (!used.has(base)) return base;
  for (let suffix = 2; suffix <= 999999999; suffix++) {
    const digits = String(suffix);
    const candidate = base.slice(0, 10 - digits.length) + digits;
    if (!used.has(candidate)) return candidate;
  }
  return '';
}
