import { localPath, PetError } from './format';

export const MAX_ZIP = 8 * 1024 * 1024;
const MAX_TOTAL = 12 * 1024 * 1024;
export function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const b of bytes) {
    crc ^= b;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}
// No filesystem extraction, external URLs, scripts, or ZIP64. Bound output while inflating.
async function inflate(
  bytes: Uint8Array,
  expected: number
): Promise<Uint8Array> {
  let stream: DecompressionStream;
  try {
    stream = new DecompressionStream('deflate-raw' as CompressionFormat);
  } catch {
    throw new PetError('unsupported');
  }
  const reader = new Blob([new Uint8Array(bytes)])
    .stream()
    .pipeThrough(stream)
    .getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    for (;;) {
      const result = await reader.read();
      if (result.done) break;
      length += result.value.length;
      if (length > expected || length > MAX_TOTAL)
        throw new PetError(
          'expanded_size',
          length,
          Math.min(expected, MAX_TOTAL)
        );
      chunks.push(result.value);
    }
  } finally {
    await reader.cancel().catch(() => undefined);
  }
  const output = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.length;
  }
  return output;
}
export async function readPetZip(
  bytes: Uint8Array
): Promise<Map<string, Uint8Array>> {
  if (bytes.length > MAX_ZIP)
    throw new PetError('zip_size', bytes.length, MAX_ZIP);
  if (bytes.length < 22) throw new PetError('invalid');
  try {
    const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let end = bytes.length - 22;
    while (
      end >= Math.max(0, bytes.length - 65557) &&
      v.getUint32(end, true) !== 0x06054b50
    )
      end--;
    if (
      end < 0 ||
      v.getUint32(end, true) !== 0x06054b50 ||
      end + 22 + v.getUint16(end + 20, true) !== bytes.length
    )
      throw new PetError('invalid');
    const count = v.getUint16(end + 10, true),
      centralSize = v.getUint32(end + 12, true),
      central = v.getUint32(end + 16, true);
    if (
      v.getUint16(end + 4, true) ||
      v.getUint16(end + 6, true) ||
      v.getUint16(end + 8, true) !== count ||
      count === 0 ||
      central + centralSize !== end
    )
      throw new PetError('invalid');
    if (count > 32) throw new PetError('entry_count', count, 32);
    const entries: {
      name: string;
      offset: number;
      size: number;
      packed: number;
      method: number;
      crc: number;
      flags: number;
    }[] = [];
    let p = central,
      total = 0;
    const names = new Set<string>();
    for (let i = 0; i < count; i++) {
      if (v.getUint32(p, true) !== 0x02014b50) throw new PetError('invalid');
      const flags = v.getUint16(p + 8, true),
        method = v.getUint16(p + 10, true);
      const packed = v.getUint32(p + 20, true),
        size = v.getUint32(p + 24, true);
      const n = v.getUint16(p + 28, true),
        extra = v.getUint16(p + 30, true),
        comment = v.getUint16(p + 32, true);
      if (p + 46 + n + extra + comment > end) throw new PetError('invalid');
      const name = new TextDecoder('utf-8', { fatal: true }).decode(
        bytes.slice(p + 46, p + 46 + n)
      );
      const isDir = name.endsWith('/');
      const mode = v.getUint32(p + 38, true) >>> 16;
      if (
        !localPath(isDir ? name.slice(0, -1) : name) ||
        names.has(name.toLowerCase()) ||
        (mode & 0xf000) === 0xa000 ||
        v.getUint16(p + 34, true) ||
        flags & ~0x808 ||
        ![0, 8].includes(method)
      )
        throw new PetError('invalid');
      names.add(name.toLowerCase());
      total += size;
      if (total > MAX_TOTAL)
        throw new PetError('expanded_size', total, MAX_TOTAL);
      if (size > MAX_ZIP) throw new PetError('entry_size', size, MAX_ZIP);
      if (name.endsWith('pet.json') && size > 16384)
        throw new PetError('manifest_size', size, 16384);
      const ratioLimit = Math.max(1024 * 1024, packed * 200);
      if (size > ratioLimit) throw new PetError('ratio_size', size, ratioLimit);
      if (!isDir)
        entries.push({
          name,
          offset: v.getUint32(p + 42, true),
          size,
          packed,
          method,
          crc: v.getUint32(p + 16, true),
          flags,
        });
      p += 46 + n + extra + comment;
    }
    if (p !== end) throw new PetError('invalid');
    const output = new Map<string, Uint8Array>();
    const ranges: [number, number][] = [];
    for (const entry of entries) {
      const q = entry.offset;
      if (
        q + 30 > central ||
        v.getUint32(q, true) !== 0x04034b50 ||
        v.getUint16(q + 6, true) !== entry.flags ||
        v.getUint16(q + 8, true) !== entry.method
      )
        throw new PetError('invalid');
      const n = v.getUint16(q + 26, true),
        extra = v.getUint16(q + 28, true),
        start = q + 30 + n + extra,
        stop = start + entry.packed;
      if (
        stop > central ||
        ranges.some(([a, b]) => q < b && stop > a) ||
        new TextDecoder('utf-8', { fatal: true }).decode(
          bytes.slice(q + 30, q + 30 + n)
        ) !== entry.name
      )
        throw new PetError('invalid');
      ranges.push([q, stop]);
      if (
        !(entry.flags & 8) &&
        (v.getUint32(q + 14, true) !== entry.crc ||
          v.getUint32(q + 18, true) !== entry.packed ||
          v.getUint32(q + 22, true) !== entry.size)
      )
        throw new PetError('invalid');
      const packed = bytes.slice(start, stop);
      const data =
        entry.method === 0 ? packed : await inflate(packed, entry.size);
      if (data.length !== entry.size || crc32(data) !== entry.crc)
        throw new PetError('invalid');
      output.set(entry.name, data);
    }
    return output;
  } catch (error) {
    if (error instanceof PetError) throw error;
    throw new PetError('invalid');
  }
}
