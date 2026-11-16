import { useState, useEffect } from 'react';
export const repoApi = {
  list: async () => handleApiResponse(await fetch('/api/repos')),
};
export async function handleApiResponse(response: Response) {
  const value = await response.json();
  if (!response.ok || value.success === false)
    throw new Error(value.message || 'Fixture API error');
  return value.data;
}

export function useShape() {
  const [data, setData] = useState<{ id: string }[]>([]);
  useEffect(() => {
    const update = () => setData([{ id: 'new-project' }]);
    window.addEventListener('fixture-projects-visible', update);
    return () => window.removeEventListener('fixture-projects-visible', update);
  }, []);
  return { data };
}
export async function refreshShapeFallback() {
  throw new Error('Synthetic delayed project cache');
}
