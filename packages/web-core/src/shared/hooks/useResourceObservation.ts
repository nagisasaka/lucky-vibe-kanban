import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { RESOURCE_POLL_MS } from '@vibe/ui/lib/resourceObservation';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

export function useResourceObservation() {
  const [visible, setVisible] = useState(
    () => document.visibilityState === 'visible'
  );
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const update = () => {
      setVisible(document.visibilityState === 'visible');
      setNow(Date.now());
    };
    document.addEventListener('visibilitychange', update);
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') setNow(Date.now());
    }, 1000);
    return () => {
      document.removeEventListener('visibilitychange', update);
      window.clearInterval(timer);
    };
  }, []);
  const query = useQuery({
    queryKey: ['resource-observation'],
    queryFn: async () => {
      const response = await makeLocalApiRequest('/api/resource-observation', {
        cache: 'no-store',
        signal: AbortSignal.timeout(5000),
      });
      if (!response.ok) throw new Error('Resource observation unavailable');
      const body = await response.json();
      if (!body.success) throw new Error('Resource observation unavailable');
      return body.data as unknown;
    },
    enabled: visible,
    staleTime: RESOURCE_POLL_MS,
    refetchInterval: visible ? RESOURCE_POLL_MS : false,
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: false,
    retry: false,
  });
  return { data: query.data, unavailable: query.isError || !visible, now };
}
