import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  frameAt,
  imageType,
  lookDirection,
  validateAtlas,
  localPath,
  parseManifest,
  PET_STATES,
} from './format';

const encode = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value));
const minimal = {
  id: 'test-pet',
  displayName: 'Test Pet',
  description: '',
  spritesheetPath: 'spritesheet.png',
};
function png(width = 1536, height = 1872) {
  const b = new Uint8Array(45);
  b.set([137, 80, 78, 71, 13, 10, 26, 10]);
  b.set(new TextEncoder().encode('IHDR'), 12);
  const v = new DataView(b.buffer);
  v.setUint32(16, width);
  v.setUint32(20, height);
  v.setUint32(8, 13);
  b.set(new TextEncoder().encode('IEND'), 37);
  return b;
}
function webp(kind: 'VP8X' | 'VP8L' | 'VP8 ', width = 1536, height = 1872) {
  const b = new Uint8Array(30),
    v = new DataView(b.buffer),
    ascii = new TextEncoder();
  b.set(ascii.encode('RIFF'));
  v.setUint32(4, 22, true);
  b.set(ascii.encode('WEBP'), 8);
  b.set(ascii.encode(kind), 12);
  v.setUint32(16, 10, true);
  if (kind === 'VP8X') {
    const w = width - 1,
      h = height - 1;
    b.set(
      [
        w & 255,
        (w >> 8) & 255,
        (w >> 16) & 255,
        h & 255,
        (h >> 8) & 255,
        (h >> 16) & 255,
      ],
      24
    );
  } else if (kind === 'VP8L') {
    b[20] = 0x2f;
    v.setUint32(21, ((height - 1) << 14) | (width - 1), true);
  } else {
    b.set([0x9d, 1, 0x2a], 23);
    v.setUint16(26, width, true);
    v.setUint16(28, height, true);
  }
  return b;
}
describe('Petdex v1 manifest', () => {
  it('accepts minimal manifests and preserves bounded attribution', () => {
    expect(
      parseManifest(
        encode({ ...minimal, author: 'Creator', license: 'CC BY 4.0' })
      )
    ).toMatchObject({ ...minimal, author: 'Creator', license: 'CC BY 4.0' });
  });
  it.each([0, 3, '1', '2', null])(
    'rejects unsupported version %s',
    (version) => {
      expect(() =>
        parseManifest(encode({ ...minimal, spriteVersionNumber: version }))
      ).toThrow('unsupported');
    }
  );
  it.each([
    'states',
    'animations',
    'frameWidth',
    'frameHeight',
    'columns',
    'rows',
  ])('rejects custom layout %s', (key) => {
    expect(() => parseManifest(encode({ ...minimal, [key]: 1 }))).toThrow(
      'unsupported'
    );
  });
  it.each([
    '../sheet.png',
    '/sheet.png',
    'https://example.com/sheet.png',
    'data:image/png,abc',
    'a\\sheet.png',
    'a/./sheet.png',
    'a//sheet.png',
  ])('rejects unsafe path %s', (path) => {
    expect(localPath(path)).toBe(false);
    expect(() =>
      parseManifest(encode({ ...minimal, spritesheetPath: path }))
    ).toThrow('invalid');
  });
  it.each([
    null,
    [],
    {},
    { ...minimal, id: '../x' },
    { ...minimal, displayName: '' },
    { ...minimal, description: 5 },
    { ...minimal, spritesheetPath: 'x.svg' },
  ])('rejects malformed manifest %#', (value) => {
    expect(() => parseManifest(encode(value))).toThrow('invalid');
  });
  it('rejects oversized, invalid JSON and invalid UTF-8', () => {
    expect(() => parseManifest(new Uint8Array(16385))).toThrow('size');
    expect(() => parseManifest(new TextEncoder().encode('{'))).toThrow(
      'invalid'
    );
    expect(() => parseManifest(new Uint8Array([255]))).toThrow('invalid');
  });
});
describe('nine native animation states', () => {
  it('keeps canonical row order and frame counts', () => {
    expect(PET_STATES.map((s) => [s.name, s.durations.length])).toEqual([
      ['idle', 6],
      ['running-right', 8],
      ['running-left', 8],
      ['waving', 4],
      ['jumping', 5],
      ['failed', 8],
      ['waiting', 6],
      ['running', 6],
      ['review', 6],
    ]);
  });
  it.each(PET_STATES)(
    '$name observes every frame boundary and loops',
    (state) => {
      let elapsed = 0;
      state.durations.forEach((duration, index) => {
        expect(frameAt(state.name, elapsed)).toBe(index);
        expect(frameAt(state.name, elapsed + duration - 1)).toBe(index);
        elapsed += duration;
      });
      expect(frameAt(state.name, elapsed)).toBe(0);
      expect(frameAt(state.name, elapsed * 5 + state.durations[0])).toBe(1);
      expect(frameAt(state.name, -1)).toBe(0);
    }
  );
});
describe('bounded image header inspection (not full decoder validation)', () => {
  it('accepts canonical PNG and all three WebP encodings', () => {
    expect(imageType(png())).toBe('image/png');
    for (const kind of ['VP8X', 'VP8L', 'VP8 '] as const)
      expect(imageType(webp(kind))).toBe('image/webp');
  });
  it.each([
    [1536, 2287],
    [1536, 1871],
    [1, 1],
    [65535, 65535],
  ])('rejects unsupported dimensions %s x %s', (w, h) => {
    expect(() => imageType(png(w, h))).toThrow('unsupported');
    expect(() => imageType(webp('VP8X', w, h))).toThrow('unsupported');
  });
  it('rejects animated WebP, truncated data, invalid signatures and bad RIFF length', () => {
    const animated = webp('VP8X');
    animated[20] = 2;
    expect(() => imageType(animated)).toThrow('unsupported');
    for (const b of [new Uint8Array(), png().slice(0, 24), new Uint8Array(40)])
      expect(() => imageType(b)).toThrow('invalid');
    const bad = webp('VP8X');
    bad[4] = 0;
    expect(() => imageType(bad)).toThrow('invalid');
  });
});

describe('static container boundaries', () => {
  function appendWebpChunk(type: string, length = 0) {
    const base = webp('VP8X'),
      b = new Uint8Array(base.length + 8 + length + (length % 2));
    b.set(base);
    b.set(new TextEncoder().encode(type), base.length);
    const v = new DataView(b.buffer);
    v.setUint32(4, b.length - 8, true);
    v.setUint32(base.length + 4, length, true);
    return b;
  }
  it.each(['acTL', 'fcTL', 'fdAT'])(
    'rejects APNG chunk %s before decoding',
    (type) => {
      const base = png(),
        b = new Uint8Array(base.length + 12);
      b.set(base.slice(0, 33));
      b.set(new TextEncoder().encode(type), 37);
      b.set(base.slice(33), 45);
      expect(() => imageType(b)).toThrow('unsupported');
    }
  );
  it.each(['ANIM', 'ANMF'])(
    'rejects WebP %s despite cleared animation flag',
    (type) => {
      expect(() => imageType(appendWebpChunk(type))).toThrow('unsupported');
    }
  );
  it('rejects PNG truncated chunks, oversized chunk lengths and trailing data', () => {
    const oversized = png();
    new DataView(oversized.buffer).setUint32(8, 0xffffffff);
    const missingEnd = png().slice(0, 33),
      trailing = new Uint8Array(46);
    trailing.set(png());
    for (const b of [oversized, missingEnd, trailing, png().slice(0, -1)])
      expect(() => imageType(b)).toThrow('invalid');
  });
  it('rejects WebP oversized chunks and missing odd-size padding', () => {
    const oversized = webp('VP8X');
    new DataView(oversized.buffer).setUint32(16, 0xffffffff, true);
    const odd = appendWebpChunk('EXIF', 1).slice(0, -1);
    new DataView(odd.buffer).setUint32(4, odd.length - 8, true);
    expect(() => imageType(oversized)).toThrow('invalid');
    expect(() => imageType(odd)).toThrow('invalid');
  });
  it('accepts bounded non-animation metadata chunks', () =>
    expect(imageType(appendWebpChunk('EXIF', 1))).toBe('image/webp'));
});

describe('extended WebP coded-image allocation bounds', () => {
  function extended(kind: 'VP8L' | 'VP8 ', width: number, height: number) {
    const canvas = webp('VP8X'),
      coded = webp(kind, width, height).slice(12);
    const result = new Uint8Array(canvas.length + coded.length);
    result.set(canvas);
    result.set(coded, canvas.length);
    new DataView(result.buffer).setUint32(4, result.length - 8, true);
    return result;
  }
  it.each(['VP8L', 'VP8 '] as const)(
    'accepts matching %s bitstream dimensions',
    (kind) => {
      expect(imageType(extended(kind, 1536, 1872))).toBe('image/webp');
    }
  );
  it.each(['VP8L', 'VP8 '] as const)(
    'rejects oversized or mismatched %s bitstreams despite a canonical canvas',
    (kind) => {
      for (const [width, height] of [
        [16383, 16383],
        [1, 1],
        [1536, 1873],
      ]) {
        expect(() => imageType(extended(kind, width, height))).toThrow(
          'unsupported'
        );
      }
    }
  );
  it.each(['VP8L', 'VP8 '] as const)(
    'rejects invalid embedded %s signatures',
    (kind) => {
      const result = extended(kind, 1536, 1872);
      result[kind === 'VP8L' ? 38 : 41] = 0;
      expect(() => imageType(result)).toThrow('invalid');
    }
  );
});

describe('v2 compatibility and static look poses', () => {
  afterEach(() => vi.unstubAllGlobals());
  it('retains explicit versions and defaults absent metadata to v1', () => {
    for (const version of [1, 2])
      expect(
        parseManifest(encode({ ...minimal, spriteVersionNumber: version }))
          .spriteVersionNumber
      ).toBe(version);
    expect(parseManifest(encode(minimal)).spriteVersionNumber).toBeUndefined();
  });
  it('checks PNG and every WebP dimension against the declared version', () => {
    expect(imageType(png(1536, 2288), 2)).toBe('image/png');
    for (const kind of ['VP8X', 'VP8L', 'VP8 '] as const) {
      expect(imageType(webp(kind, 1536, 2288), 2)).toBe('image/webp');
      expect(() => imageType(webp(kind), 2)).toThrow('unsupported');
      expect(() => imageType(webp(kind, 1536, 2288), 1)).toThrow('unsupported');
    }
    expect(() => imageType(png(), 2)).toThrow('unsupported');
    expect(() => imageType(png(1536, 2288))).toThrow('unsupported');
  });
  it('maps all sixteen clockwise angles, wraps north and keeps center neutral', () => {
    for (let index = 0; index < 16; index++) {
      const radians = (index * Math.PI) / 8;
      expect(
        lookDirection(Math.sin(radians) * 100, -Math.cos(radians) * 100, 16)
      ).toBe(index);
    }
    expect(lookDirection(-1, -100, 16)).toBe(0);
    expect(lookDirection(0, 0, 16)).toBeNull();
    expect(lookDirection(16, 0, 16)).toBeNull();
    expect(lookDirection(NaN, 0, 16)).toBeNull();
  });
  it('enforces v2 cells without imposing new alpha rules on legacy v1', () => {
    for (const version of [1, 2] as const) {
      const height = version === 2 ? 2288 : 1872;
      const pixels = new Uint8ClampedArray(1536 * height * 4);
      for (let row = 0; row < height / 208; row++) {
        const frames = row < 9 ? PET_STATES[row].durations.length : 8;
        for (let col = 0; col < frames; col++)
          pixels[((row * 208 + 100) * 1536 + col * 192 + 96) * 4 + 3] = 255;
      }
      const canvas = {
        width: 0,
        height: 0,
        getContext: () => ({
          drawImage: vi.fn(),
          getImageData: () => ({ data: pixels }),
        }),
      };
      vi.stubGlobal('document', { createElement: () => canvas });
      const image = {} as CanvasImageSource;
      expect(() => validateAtlas(image, 1536, height, version)).not.toThrow();
      pixels[(100 * 1536 + 96) * 4 + 3] = 0;
      if (version === 2)
        expect(() => validateAtlas(image, 1536, height, version)).toThrow(
          'invalid'
        );
      else
        expect(() => validateAtlas(image, 1536, height, version)).not.toThrow();
      pixels[(100 * 1536 + 96) * 4 + 3] = 255;
      // Producer v2's neutral pose is optional, not the seventh idle frame.
      pixels[(100 * 1536 + 6 * 192 + 96) * 4 + 3] = 1;
      expect(() => validateAtlas(image, 1536, height, version)).not.toThrow();
      expect(PET_STATES[0].durations).toHaveLength(6);
      pixels[(100 * 1536 + 7 * 192 + 96) * 4 + 3] = 1;
      if (version === 2)
        expect(() => validateAtlas(image, 1536, height, version)).toThrow(
          'invalid'
        );
      else
        expect(() => validateAtlas(image, 1536, height, version)).not.toThrow();
      expect(canvas.width).toBe(0);
    }
  });
});
