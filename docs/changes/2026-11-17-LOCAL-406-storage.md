# LOCAL-406: repository artifact ownership

Shared `persistent` is reserved for user-retained outputs. The retired startup
importer no longer reads legacy Change Manifest files. Applied migrations and
`repository_manifest_imports` rows remain unchanged. Final Manifests continue to
use the instance DB; no file fallback, deletion or live migration is performed.

This change does not authorise deployment or disposal of existing files. Before
upgrading, stop all old server/agent hosts and settle unfinished coding and Wiki
operations using their original version and fixed paths. Do not run the old and
new lock implementations concurrently. Keep the old artifacts and matching DB
backup until recovery is independently verified. A clean Git checkout or a
successful AgentRun alone does not prove that restoration/publication finished.
