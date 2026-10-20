import { LVK_GITHUB_REPOSITORY } from '@vibe/ui/lib/repository';
import { useQuery } from '@tanstack/react-query';

async function fetchGitHubStars(): Promise<number | null> {
  try {
    const res = await fetch(
      `https://api.github.com/repos/${LVK_GITHUB_REPOSITORY}`,
      { cache: 'no-store' }
    );

    if (!res.ok) {
      console.warn(`GitHub API error: ${res.status}`);
      return null;
    }

    const data = await res.json();
    if (
      Number.isSafeInteger(data?.stargazers_count) &&
      data.stargazers_count >= 0
    ) {
      return data.stargazers_count;
    }

    return null;
  } catch (error) {
    console.warn('Failed to fetch GitHub stars:', error);
    return null;
  }
}

export function useGitHubStars() {
  return useQuery({
    queryKey: ['github-stars', LVK_GITHUB_REPOSITORY],
    queryFn: fetchGitHubStars,
    refetchInterval: 10 * 60 * 1000,
    staleTime: 10 * 60 * 1000,
    retry: false,
    refetchOnMount: false,
    placeholderData: (previousData) => previousData,
  });
}
