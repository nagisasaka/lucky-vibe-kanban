import { Unzip, UnzipInflate } from 'fflate';
import { localPath, PetError } from './format';

export const MAX_ZIP = 8 * 1024 * 1024;
const MAX_TOTAL = 12 * 1024 * 1024;

// ZIP interpretation belongs to fflate. Only application limits and names are
// checked here; CRC, Unix attributes and header consistency are not our contract.
export async function readPetZip(
  bytes: Uint8Array
): Promise<Map<string, Uint8Array>> {
  if (bytes.length > MAX_ZIP)
    throw new PetError('zip_size', bytes.length, MAX_ZIP);
  const output = new Map<string, Uint8Array>();
  const names = new Set<string>();
  let total = 0;
  let pending = 0;
  try {
    const archive = new Unzip((file) => {
      const directory = file.name.endsWith('/');
      const name = directory ? file.name.slice(0, -1) : file.name;
      if (!localPath(name) || names.has(name.toLowerCase()))
        throw new PetError('invalid');
      names.add(name.toLowerCase());
      if (names.size > 32) throw new PetError('entry_count', names.size, 32);
      if (directory) return;
      if (file.originalSize !== undefined && file.originalSize > MAX_ZIP)
        throw new PetError('entry_size', file.originalSize, MAX_ZIP);
      const chunks: Uint8Array[] = [];
      let length = 0;
      pending++;
      file.ondata = (error, chunk, final) => {
        if (error) throw error;
        length += chunk.length;
        total += chunk.length;
        if (length > MAX_ZIP) throw new PetError('entry_size', length, MAX_ZIP);
        if (total > MAX_TOTAL)
          throw new PetError('expanded_size', total, MAX_TOTAL);
        if (file.name.endsWith('pet.json') && length > 16384)
          throw new PetError('manifest_size', length, 16384);
        // Streaming-created archives may not declare sizes; actual byte limits
        // above always apply, regardless of missing or incorrect metadata.
        if (file.size !== undefined) {
          const limit = Math.max(1024 * 1024, file.size * 200);
          if (length > limit) throw new PetError('ratio_size', length, limit);
        }
        chunks.push(chunk);
        if (final) {
          const data = new Uint8Array(length);
          let offset = 0;
          for (const chunk of chunks) {
            data.set(chunk, offset);
            offset += chunk.length;
          }
          output.set(file.name, data);
          pending--;
        }
      };
      file.start();
    });
    archive.register(UnzipInflate);
    for (let offset = 0; offset < bytes.length; offset += 1024)
      archive.push(
        bytes.subarray(offset, offset + 1024),
        offset + 1024 >= bytes.length
      );
    if (pending || !output.size) throw new PetError('invalid');
    return output;
  } catch (error) {
    if (error instanceof PetError) throw error;
    throw new PetError('invalid');
  }
}
