import { deflateRawSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import { crc32, MAX_ZIP, readPetZip } from './zip';

type Entry = {
  name: string;
  data?: Uint8Array;
  method?: number;
  flags?: number;
  mode?: number;
  claimedSize?: number;
  crc?: number;
};
const text = (s: string) => new TextEncoder().encode(s);
// Construct local and central records independently of the parser under test.
function zip(entries: Entry[]) {
  const records: Uint8Array[] = [],
    central: Uint8Array[] = [];
  let offset = 0;
  for (const e of entries) {
    const name = text(e.name),
      data = e.data ?? text('payload'),
      method = e.method ?? 0;
    const packed = method === 8 ? new Uint8Array(deflateRawSync(data)) : data;
    const size = e.claimedSize ?? data.length,
      crc = e.crc ?? crc32(data);
    const l = new Uint8Array(30 + name.length + packed.length),
      lv = new DataView(l.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 20, true);
    lv.setUint16(6, e.flags ?? 0, true);
    lv.setUint16(8, method, true);
    lv.setUint32(14, crc, true);
    lv.setUint32(18, packed.length, true);
    lv.setUint32(22, size, true);
    lv.setUint16(26, name.length, true);
    l.set(name, 30);
    l.set(packed, 30 + name.length);
    const c = new Uint8Array(46 + name.length),
      cv = new DataView(c.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(4, 0x314, true);
    cv.setUint16(6, 20, true);
    cv.setUint16(8, e.flags ?? 0, true);
    cv.setUint16(10, method, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, packed.length, true);
    cv.setUint32(24, size, true);
    cv.setUint16(28, name.length, true);
    cv.setUint32(38, (e.mode ?? 0) << 16, true);
    cv.setUint32(42, offset, true);
    c.set(name, 46);
    records.push(l);
    central.push(c);
    offset += l.length;
  }
  const centralSize = central.reduce((s, c) => s + c.length, 0),
    end = new Uint8Array(22),
    ev = new DataView(end.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, entries.length, true);
  ev.setUint16(10, entries.length, true);
  ev.setUint32(12, centralSize, true);
  ev.setUint32(16, offset, true);
  const result = new Uint8Array(offset + centralSize + 22);
  let p = 0;
  for (const part of [...records, ...central, end]) {
    result.set(part, p);
    p += part.length;
  }
  return result;
}
describe('bounded ZIP reader', () => {
  it('checks CRC against a published check vector', () =>
    expect(crc32(text('123456789'))).toBe(0xcbf43926));
  it('reads stored entries and nested package paths without filesystem extraction', async () => {
    const result = await readPetZip(
      zip([
        { name: 'rabbit/pet.json', data: text('{}') },
        { name: 'rabbit/spritesheet.png' },
      ])
    );
    expect([...result.keys()]).toEqual([
      'rabbit/pet.json',
      'rabbit/spritesheet.png',
    ]);
    expect(result.get('rabbit/pet.json')).toEqual(text('{}'));
  });
  it('reads deflated entries using the native decompression stream', async () => {
    const data = text('hello pet '.repeat(50));
    expect(
      (await readPetZip(zip([{ name: 'pet.json', data, method: 8 }]))).get(
        'pet.json'
      )
    ).toEqual(data);
  });
  it.each([
    '../pet.json',
    '/pet.json',
    'a/../../pet.json',
    'a\\pet.json',
    'https://evil.test/pet.json',
    'C:/pet.json',
    'a//pet.json',
  ])('rejects unsafe entry %s', async (name) => {
    await expect(readPetZip(zip([{ name }]))).rejects.toThrow('invalid');
  });
  it('rejects duplicate and case-folded duplicate paths', async () => {
    for (const name of ['pet.json', 'PET.JSON'])
      await expect(
        readPetZip(zip([{ name: 'pet.json' }, { name }]))
      ).rejects.toThrow('invalid');
  });
  it.each([{ mode: 0xa1ff }, { flags: 1 }, { flags: 0x40 }, { method: 12 }])(
    'rejects symlinks, encryption and unsupported methods %#',
    async (options) => {
      await expect(
        readPetZip(zip([{ name: 'pet.json', ...options }]))
      ).rejects.toThrow('invalid');
    }
  );
  it('rejects bad CRC, inconsistent claimed length and truncated ZIP', async () => {
    await expect(
      readPetZip(zip([{ name: 'pet.json', crc: 0 }]))
    ).rejects.toThrow('invalid');
    await expect(
      readPetZip(zip([{ name: 'pet.json', claimedSize: 100 }]))
    ).rejects.toThrow('invalid');
    await expect(
      readPetZip(zip([{ name: 'pet.json' }]).slice(0, -1))
    ).rejects.toThrow('invalid');
    await expect(readPetZip(new Uint8Array(21))).rejects.toThrow('invalid');
  });
  it('rejects mismatched local names and overlapping local records', async () => {
    const name = zip([{ name: 'pet.json' }]);
    name[30] = 120;
    await expect(readPetZip(name)).rejects.toThrow('invalid');
    const overlap = zip([{ name: 'pet.json' }, { name: 'sheet.png' }]),
      v = new DataView(overlap.buffer);
    const end = overlap.length - 22,
      center = v.getUint32(end + 16, true),
      next = center + 46 + 'pet.json'.length;
    v.setUint32(next + 42, 0, true);
    await expect(readPetZip(overlap)).rejects.toThrow('invalid');
  });
  it('bounds archive size, manifest size, entry count, total output and expansion ratio', async () => {
    await expect(readPetZip(new Uint8Array(MAX_ZIP + 1))).rejects.toThrow(
      'size'
    );
    await expect(
      readPetZip(zip([{ name: 'pet.json', data: new Uint8Array(16385) }]))
    ).rejects.toThrow('size');
    await expect(
      readPetZip(
        zip(Array.from({ length: 33 }, (_, i) => ({ name: `file${i}` })))
      )
    ).rejects.toThrow('invalid');
    await expect(
      readPetZip(
        zip([
          { name: 'sheet.png', claimedSize: 7 * 1024 * 1024 },
          { name: 'other.png', claimedSize: 7 * 1024 * 1024 },
        ])
      )
    ).rejects.toThrow('size');
    await expect(
      readPetZip(
        zip([
          {
            name: 'sheet.png',
            data: new Uint8Array(2 * 1024 * 1024),
            method: 8,
          },
        ])
      )
    ).rejects.toThrow('size');
  });
  it('bounds actual inflater output even if a bomb lies about its output size', async () => {
    await expect(
      readPetZip(
        zip([
          {
            name: 'sheet.png',
            data: new Uint8Array(100000),
            method: 8,
            claimedSize: 10,
          },
        ])
      )
    ).rejects.toThrow('size');
  });
});
