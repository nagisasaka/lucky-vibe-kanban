import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_PET,
  DEFAULT_PREFERENCES,
  importPet,
  restorePreferences,
} from './library';
import { crc32, MAX_ZIP } from './zip';

const encode = (s: string) => new TextEncoder().encode(s);
const manifest = {
  id: 'test',
  displayName: 'Test',
  description: '',
  spritesheetPath: 'sheet.png',
  author: 'Creator',
  license: 'CC BY 4.0',
};
const key = '0a000000-0000-0000-0000-000000000001';
const attachmentId = '0a000000-0000-0000-0000-000000000002';
function png(height = 1872) {
  const b = new Uint8Array(45),
    v = new DataView(b.buffer);
  b.set([137, 80, 78, 71, 13, 10, 26, 10]);
  b.set(encode('IHDR'), 12);
  v.setUint32(16, 1536);
  v.setUint32(20, height);
  v.setUint32(8, 13);
  b.set(new TextEncoder().encode('IEND'), 37);
  return b;
}
function fixture(entries: [string, Uint8Array][]) {
  const parts: Uint8Array[] = [],
    central: Uint8Array[] = [];
  let offset = 0;
  for (const [name, data] of entries) {
    const n = encode(name),
      crc = crc32(data),
      l = new Uint8Array(30 + n.length + data.length),
      lv = new DataView(l.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint32(14, crc, true);
    lv.setUint32(18, data.length, true);
    lv.setUint32(22, data.length, true);
    lv.setUint16(26, n.length, true);
    l.set(n, 30);
    l.set(data, 30 + n.length);
    const c = new Uint8Array(46 + n.length),
      cv = new DataView(c.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, data.length, true);
    cv.setUint32(24, data.length, true);
    cv.setUint16(28, n.length, true);
    cv.setUint32(42, offset, true);
    c.set(n, 46);
    parts.push(l);
    central.push(c);
    offset += l.length;
  }
  const end = new Uint8Array(22),
    ev = new DataView(end.buffer),
    centralSize = central.reduce((s, c) => s + c.length, 0);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, entries.length, true);
  ev.setUint16(10, entries.length, true);
  ev.setUint32(12, centralSize, true);
  ev.setUint32(16, offset, true);
  const b = new Uint8Array(offset + centralSize + 22);
  let p = 0;
  for (const part of [...parts, ...central, end]) {
    b.set(part, p);
    p += part.length;
  }
  return new File([b], 'pet.zip', { type: 'application/zip' });
}
const validEntries = (root = ''): [string, Uint8Array][] => [
  [`${root}pet.json`, encode(JSON.stringify(manifest))],
  [`${root}sheet.png`, png()],
];
const saved = () => ({
  key,
  attachmentId,
  manifest: { ...manifest },
  originalManifest: JSON.stringify(manifest),
  licenseText: 'License text',
});
afterEach(() => vi.unstubAllGlobals());
describe('restoring shared pet preferences', () => {
  it.each([
    undefined,
    null,
    42,
    'bad',
    [],
    {},
    { pets: 'bad' },
    { pets: new Array(13).fill(saved()) },
  ])('safely defaults corrupt preferences %#', (value) => {
    expect(restorePreferences(value)).toEqual(DEFAULT_PREFERENCES);
  });
  it('restores selection, visibility, attribution and original metadata', () => {
    const value = { selected: key, hidden: true, pets: [saved()] };
    expect(restorePreferences(value)).toEqual(value);
  });
  it('defaults stale selection and treats visibility as a strict boolean', () => {
    expect(
      restorePreferences({
        selected: 'missing',
        hidden: 'true',
        pets: [saved()],
      })
    ).toMatchObject({ selected: DEFAULT_PET, hidden: false });
  });
  it.each([
    null,
    { ...saved(), key: '../bad' },
    { ...saved(), attachmentId: 'bad' },
    {
      ...saved(),
      manifest: { ...manifest, spritesheetPath: 'https://evil.test/a.png' },
    },
    { ...saved(), originalManifest: 'a'.repeat(16385) },
    { ...saved(), licenseText: 'a'.repeat(65537) },
  ])('drops corrupt pet records while preserving valid records %#', (bad) => {
    const input = { selected: key, hidden: false, pets: [bad, saved()] };
    expect(restorePreferences(input)).toEqual({
      selected: key,
      hidden: false,
      pets: [saved()],
    });
    expect(input.pets).toHaveLength(2);
  });
});
describe('isolated local pet import', () => {
  function decoder(width = 1536, height = 1872) {
    const close = vi.fn();
    const pixels = new Uint8ClampedArray(1536 * height * 4);
    const frames = [6, 8, 8, 4, 5, 8, 6, 6, 6, 8, 8];
    for (let row = 0; row < height / 208; row++)
      for (let col = 0; col < frames[row]; col++)
        pixels[((row * 208 + 1) * 1536 + col * 192 + 1) * 4 + 3] = 255;
    vi.stubGlobal('document', {
      createElement: () => ({
        width: 0,
        height: 0,
        getContext: () => ({
          drawImage: vi.fn(),
          getImageData: () => ({ data: pixels }),
        }),
      }),
    });
    const decode = vi.fn().mockResolvedValue({ width, height, close });
    vi.stubGlobal('createImageBitmap', decode);
    return { close, decode };
  }
  it('imports nested packages and preserves exact original manifest and license text', async () => {
    const { close, decode } = decoder(),
      root = 'rabbit/';
    const original =
      JSON.stringify(
        {
          ...manifest,
          customAttribution: { source: 'https://example.com/author' },
        },
        null,
        2
      ) + '\n';
    const entries = validEntries(root);
    entries[0][1] = encode(original);
    entries.push([root + 'LICENSE.md', encode('© Creator\nCC BY 4.0\n')]);
    const result = await importPet(fixture(entries));
    expect(result.manifest).toEqual(manifest);
    expect(result.originalManifest).toBe(original);
    expect(result.licenseText).toBe('© Creator\nCC BY 4.0\n');
    expect(result.image.type).toBe('image/png');
    expect(result.image.name).toBe('pet.png');
    expect(result.image.size).toBe(45);
    expect(decode).toHaveBeenCalledOnce();
    expect(close).toHaveBeenCalledOnce();
  });
  it('accepts a package without a separate license document', async () => {
    decoder();
    expect((await importPet(fixture(validEntries()))).licenseText).toBe('');
  });
  it('rejects decode errors and leaves existing preferences untouched', async () => {
    vi.stubGlobal(
      'createImageBitmap',
      vi.fn().mockRejectedValue(new Error('broken image'))
    );
    const prefs = { selected: key, hidden: false, pets: [saved()] },
      before = JSON.stringify(prefs);
    await expect(importPet(fixture(validEntries()))).rejects.toThrow('invalid');
    expect(JSON.stringify(prefs)).toBe(before);
  });
  it('checks actual decoded dimensions and closes rejected bitmaps', async () => {
    const { close } = decoder(1, 1);
    await expect(importPet(fixture(validEntries()))).rejects.toThrow(
      'unsupported'
    );
    expect(close).toHaveBeenCalledOnce();
  });
  it('imports v2 and restores its version without dropping older v1 records', async () => {
    decoder(1536, 2288);
    const v2 = { ...manifest, spriteVersionNumber: 2 };
    const result = await importPet(
      fixture([
        ['pet.json', encode(JSON.stringify(v2))],
        ['sheet.png', png(2288)],
      ])
    );
    expect(result.manifest).toEqual(v2);
    const record = {
      ...saved(),
      manifest: result.manifest,
      originalManifest: result.originalManifest,
    };
    expect(
      restorePreferences({ selected: key, hidden: false, pets: [record] })
        .pets[0].manifest.spriteVersionNumber
    ).toBe(2);
    expect(
      restorePreferences({ selected: key, hidden: false, pets: [saved()] })
        .pets[0].manifest.spriteVersionNumber
    ).toBeUndefined();
    await expect(
      importPet(
        fixture([
          ['pet.json', encode(JSON.stringify(manifest))],
          ['sheet.png', png(2288)],
        ])
      )
    ).rejects.toThrow('unsupported');
  });
  it('rejects zero/multiple manifests and missing images before decoding', async () => {
    const { decode } = decoder();
    for (const entries of [
      [['other.txt', encode('x')]],
      [['pet.json', encode(JSON.stringify(manifest))]],
      [...validEntries(), ['other/pet.json', encode(JSON.stringify(manifest))]],
    ] as [string, Uint8Array][][]) {
      await expect(importPet(fixture(entries))).rejects.toThrow('invalid');
    }
    expect(decode).not.toHaveBeenCalled();
  });
  it('rejects oversized input and license documents', async () => {
    decoder();
    await expect(
      importPet(new File([new Uint8Array(MAX_ZIP + 1)], 'large.zip'))
    ).rejects.toThrow('size');
    await expect(
      importPet(
        fixture([...validEntries(), ['LICENSE', new Uint8Array(65537)]])
      )
    ).rejects.toThrow('size');
  });
  it('does not borrow an unrelated package license', async () => {
    decoder();
    expect(
      (
        await importPet(
          fixture([
            ...validEntries('rabbit/'),
            ['LICENSE', encode('unrelated')],
          ])
        )
      ).licenseText
    ).toBe('');
  });
});
