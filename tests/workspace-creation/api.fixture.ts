// Synthetic API: no server, database, provider or user repository is touched.
export class ApiError extends Error {
  status = 404;
}
const repos = ['a', 'b', 'outside'].map((id) => ({
  id,
  name: id === 'outside' ? 'a' : id,
  display_name: `Repo ${id}`,
  path: `/synthetic/${id}`,
  default_target_branch: 'main',
}));
export const repoApi = {
  list: async () => repos,
  getBranches: async () => [
    { name: 'main', is_current: true, is_remote: false },
    { name: 'develop', is_current: false, is_remote: false },
  ],
};
export const scratchApi = {
  get: async () => ({
    payload: {
      type: 'PROJECT_REPO_DEFAULTS',
      data: {
        repos: ['a', 'b'].map((repo_id) => ({ repo_id, target_branch: '' })),
        directory_path: null,
      },
    },
  }),
};
export const fileSystemApi = {
  inspectDirectory: async () => ({ repo: repos[2] }),
};
