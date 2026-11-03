import { imageType, parseManifest, PetError, type PetManifest } from './format';
import { MAX_ZIP, readPetZip } from './zip';

export type SavedPet = {
  key: string;
  attachmentId: string;
  manifest: PetManifest;
  originalManifest: string;
  licenseText: string;
};
export type PetPreferences = {
  selected: string;
  hidden: boolean;
  pets: SavedPet[];
};
export const DEFAULT_PET = 'lvk-rabbit';
export const DEFAULT_PREFERENCES: PetPreferences = {
  selected: DEFAULT_PET,
  hidden: false,
  pets: [],
};
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function restorePreferences(value: unknown): PetPreferences {
  try {
    if (!value || typeof value !== 'object') return DEFAULT_PREFERENCES;
    const p = value as PetPreferences;
    if (!Array.isArray(p.pets) || p.pets.length > 12)
      return DEFAULT_PREFERENCES;
    const pets = p.pets.filter((pet) => {
      try {
        if (
          !uuid.test(pet.key) ||
          !uuid.test(pet.attachmentId) ||
          typeof pet.originalManifest !== 'string' ||
          pet.originalManifest.length > 16384 ||
          typeof pet.licenseText !== 'string' ||
          pet.licenseText.length > 65536
        )
          return false;
        parseManifest(new TextEncoder().encode(JSON.stringify(pet.manifest)));
        return true;
      } catch {
        return false;
      }
    });
    return {
      selected: pets.some((pet) => pet.key === p.selected)
        ? p.selected
        : DEFAULT_PET,
      hidden: p.hidden === true,
      pets,
    };
  } catch {
    return DEFAULT_PREFERENCES;
  }
}
export async function importPet(file: File): Promise<{
  manifest: PetManifest;
  originalManifest: string;
  licenseText: string;
  image: File;
}> {
  if (file.size > MAX_ZIP) throw new PetError('size');
  const entries = await readPetZip(new Uint8Array(await file.arrayBuffer()));
  const manifests = [...entries.keys()].filter(
    (name) => name === 'pet.json' || name.endsWith('/pet.json')
  );
  if (manifests.length !== 1) throw new PetError('invalid');
  const path = manifests[0],
    root = path.slice(0, -8);
  const raw = entries.get(path)!;
  const manifest = parseManifest(raw);
  const image = entries.get(root + manifest.spritesheetPath);
  if (!image) throw new PetError('invalid');
  const mime = imageType(image);
  const blob = new Blob([new Uint8Array(image)], { type: mime });
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(blob);
  } catch {
    throw new PetError('invalid');
  }
  try {
    if (bitmap.width !== 1536 || bitmap.height !== 1872)
      throw new PetError('unsupported');
  } finally {
    bitmap.close();
  }
  const license = [...entries].find(
    ([name]) =>
      /^(license|licence|copying)(\.txt|\.md)?$/i.test(
        name.slice(root.length)
      ) && name.startsWith(root)
  );
  if (license && license[1].length > 65536) throw new PetError('size');
  return {
    manifest,
    originalManifest: new TextDecoder().decode(raw),
    licenseText: license ? new TextDecoder().decode(license[1]) : '',
    image: new File([blob], mime === 'image/png' ? 'pet.png' : 'pet.webp', {
      type: mime,
    }),
  };
}
