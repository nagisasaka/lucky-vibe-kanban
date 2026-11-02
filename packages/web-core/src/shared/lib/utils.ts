import { type ClassValue, clsx } from 'clsx';
// import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  // TODO: Re-enable twMerge after migration to tailwind v4
  // Doesn't support de-duplicating custom classes, eg text-brand and text-base
  // return twMerge(clsx(inputs));
  return clsx(inputs);
}

/** Play notification sounds using the browser's audio implementation. */
export async function playSound(url: string): Promise<void> {
  const audio = new Audio(url);
  await audio.play();
}
