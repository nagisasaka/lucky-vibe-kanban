# LVK development

## LVK developer-only development-server self-restart

- `self_restart` is a required developer operations capability for LVK developers to switch development-server artifacts and restart that development instance. Keep this role distinct from ordinary users' stable-container image updates and host-level recovery of an already stopped LVK server; a stopped server cannot receive MCP requests. Do not require ordinary users to configure this capability.
- The dedicated `self_restart` path invokes the shared switching engine asynchronously; it is not ordinary `run_process` execution. Before offering to use it, verify actual tool exposure, the target instance, its configured engine/configuration, and the required instance-scoped grant. Do not infer readiness from merged code or bypass ordinary execution restrictions.
- Obtain the applicable deployment/restart authorization, distinguish acceptance from completion, and retain the operation ID. Use `get_self_restart_operation` for the persistent switching outcome and, after reconnection, verify the running revision and health before reporting success. Report unverified UI, authentication, or real-agent behavior separately.
