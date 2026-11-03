// Petdex native timing, pinned source and compatibility scope in docs/development/pets.md.
export const PET_STATES = [
  { name: 'idle', durations: [280, 110, 110, 140, 140, 320] },
  {
    name: 'running-right',
    durations: [120, 120, 120, 120, 120, 120, 120, 220],
  },
  { name: 'running-left', durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  { name: 'waving', durations: [140, 140, 140, 280] },
  { name: 'jumping', durations: [140, 140, 140, 140, 280] },
  { name: 'failed', durations: [140, 140, 140, 140, 140, 140, 140, 240] },
  { name: 'waiting', durations: [150, 150, 150, 150, 150, 260] },
  { name: 'running', durations: [120, 120, 120, 120, 120, 220] },
  { name: 'review', durations: [150, 150, 150, 150, 150, 280] },
] as const;
export type PetState = (typeof PET_STATES)[number]['name'];
export type PetManifest = {
  id: string;
  displayName: string;
  description: string;
  spritesheetPath: string;
  author?: string;
  license?: string;
};
export class PetError extends Error {
  constructor(public code: 'invalid' | 'size' | 'unsupported' | 'storage') {
    super(code);
  }
}
export function localPath(path: string): boolean {
  return (
    path.length <= 200 &&
    /^[a-zA-Z0-9_. /-]+$/.test(path) &&
    !path.startsWith('/') &&
    path.split('/').every((p) => p && p !== '.' && p !== '..')
  );
}
export function parseManifest(bytes: Uint8Array): PetManifest {
  if (bytes.length > 16384) throw new PetError('size');
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  } catch {
    throw new PetError('invalid');
  }
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new PetError('invalid');
  const m = value as Record<string, unknown>;
  if (m.spriteVersionNumber !== undefined && m.spriteVersionNumber !== 1)
    throw new PetError('unsupported');
  // Custom frame/state layouts are not part of the supported fixed v1 atlas.
  for (const key of [
    'states',
    'animations',
    'frameWidth',
    'frameHeight',
    'columns',
    'rows',
  ]) {
    if (key in m) throw new PetError('unsupported');
  }
  if (
    typeof m.id !== 'string' ||
    !/^[a-zA-Z0-9_-]{1,100}$/.test(m.id) ||
    typeof m.displayName !== 'string' ||
    !m.displayName.trim() ||
    m.displayName.length > 120 ||
    typeof m.description !== 'string' ||
    m.description.length > 2000 ||
    typeof m.spritesheetPath !== 'string' ||
    !localPath(m.spritesheetPath) ||
    !/\.(png|webp)$/i.test(m.spritesheetPath)
  )
    throw new PetError('invalid');
  const metadata = (key: string) =>
    typeof m[key] === 'string' && (m[key] as string).length <= 2000
      ? (m[key] as string)
      : undefined;
  return {
    id: m.id,
    displayName: m.displayName,
    description: m.description,
    spritesheetPath: m.spritesheetPath,
    author: metadata('author'),
    license: metadata('license'),
  };
}
export function frameAt(state: PetState, elapsed: number): number {
  const durations = PET_STATES.find((s) => s.name === state)!.durations;
  let t = Math.max(0, elapsed) % durations.reduce<number>((a, b) => a + b, 0);
  for (let i = 0; i < durations.length; i++) {
    if (t < durations[i]) return i;
    t -= durations[i];
  }
  return 0;
}
// Read dimensions before handing compressed images to the browser decoder.
export function imageType(bytes: Uint8Array): 'image/png' | 'image/webp' {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const ascii = (start: number, end: number) =>
    String.fromCharCode(...bytes.slice(start, end));
  let width = 0,
    height = 0;
  if (
    bytes.length >= 33 &&
    bytes
      .slice(0, 8)
      .every((b, i) => b === [137, 80, 78, 71, 13, 10, 26, 10][i]) &&
    ascii(12, 16) === 'IHDR'
  ) {
    // APNG must never reach the browser as an animated image. Walk real chunk
    // boundaries instead of searching compressed bytes for a marker string.
    let chunk = 8;
    let ended = false;
    while (chunk < bytes.length) {
      if (bytes.length - chunk < 12) throw new PetError('invalid');
      const length = v.getUint32(chunk);
      const end = chunk + 12 + length;
      if (end > bytes.length) throw new PetError('invalid');
      const type = ascii(chunk + 4, chunk + 8);
      if (['acTL', 'fcTL', 'fdAT'].includes(type))
        throw new PetError('unsupported');
      if (type === 'IHDR' && (chunk !== 8 || length !== 13))
        throw new PetError('invalid');
      if (type === 'IEND') {
        if (length !== 0 || end !== bytes.length) throw new PetError('invalid');
        ended = true;
      }
      chunk = end;
    }
    if (!ended) throw new PetError('invalid');
    width = v.getUint32(16);
    height = v.getUint32(20);
    if (width !== 1536 || height !== 1872) throw new PetError('unsupported');
    return 'image/png';
  }
  if (
    bytes.length >= 30 &&
    ascii(0, 4) === 'RIFF' &&
    ascii(8, 12) === 'WEBP' &&
    v.getUint32(4, true) + 8 === bytes.length
  ) {
    // Do not trust VP8X's animation flag alone: reject animation chunks even
    // when a container lies about that flag, and bound every chunk including padding.
    let chunk = 12;
    while (chunk < bytes.length) {
      if (bytes.length - chunk < 8) throw new PetError('invalid');
      const length = v.getUint32(chunk + 4, true);
      const end = chunk + 8 + length + (length % 2);
      if (end > bytes.length) throw new PetError('invalid');
      const type = ascii(chunk, chunk + 4);
      if (type === 'ANIM' || type === 'ANMF') throw new PetError('unsupported');
      // Extended canvas dimensions do not bound the encoded image's allocation.
      // Inspect each actual bitstream before handing it to the browser decoder.
      if (type === 'VP8L' || type === 'VP8 ') {
        const data = chunk + 8;
        let codedWidth: number, codedHeight: number;
        if (type === 'VP8L') {
          if (length < 5 || bytes[data] !== 0x2f) throw new PetError('invalid');
          const bits = v.getUint32(data + 1, true);
          codedWidth = (bits & 0x3fff) + 1;
          codedHeight = ((bits >>> 14) & 0x3fff) + 1;
        } else {
          if (
            length < 10 ||
            bytes[data + 3] !== 0x9d ||
            bytes[data + 4] !== 1 ||
            bytes[data + 5] !== 0x2a
          )
            throw new PetError('invalid');
          codedWidth = v.getUint16(data + 6, true) & 0x3fff;
          codedHeight = v.getUint16(data + 8, true) & 0x3fff;
        }
        if (codedWidth !== 1536 || codedHeight !== 1872)
          throw new PetError('unsupported');
      }
      chunk = end;
    }
    const kind = ascii(12, 16);
    const firstLength = v.getUint32(16, true);
    if (
      (kind === 'VP8X' && firstLength !== 10) ||
      (kind === 'VP8L' && firstLength < 5) ||
      (kind === 'VP8 ' && firstLength < 10)
    )
      throw new PetError('invalid');
    if (kind === 'VP8X') {
      // Animation containers are unnecessary and multiply decoding costs.
      if (bytes[20] & 2) throw new PetError('unsupported');
      width = 1 + bytes[24] + (bytes[25] << 8) + (bytes[26] << 16);
      height = 1 + bytes[27] + (bytes[28] << 8) + (bytes[29] << 16);
    } else if (kind === 'VP8L' && bytes[20] === 0x2f) {
      const bits = v.getUint32(21, true);
      width = (bits & 0x3fff) + 1;
      height = ((bits >>> 14) & 0x3fff) + 1;
    } else if (
      kind === 'VP8 ' &&
      bytes[23] === 0x9d &&
      bytes[24] === 1 &&
      bytes[25] === 0x2a
    ) {
      width = v.getUint16(26, true) & 0x3fff;
      height = v.getUint16(28, true) & 0x3fff;
    } else throw new PetError('invalid');
    if (width !== 1536 || height !== 1872) throw new PetError('unsupported');
    return 'image/webp';
  }
  throw new PetError('invalid');
}
