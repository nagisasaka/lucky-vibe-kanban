# Repository operational state: one-time development import

This pre-release import is for the single registered LVK repository. It is an
explicit operator command, not a startup audit, per-Repo quarantine system or
ongoing migration service. Do not run it from a normal coding task against shared
instance data.

1. Obtain cutover approval and coordinate the instance using the existing
   [switch procedure](switch/README.md). Settle Runs, Wiki maintenance,
   integrations and reservations with the old version, then stop the old server.
   Retain the old binary and a consistent backup of the instance DB and persistent
   folder using that procedure. Do not let old/new writers overlap.
2. With the same `LVK_INSTANCE_DIR` and storage configuration as the stopped
   instance, explicitly run the new binary:

   ```sh
   /path/to/new/server migrate-repository-state <repository-uuid>
   ```

   This applies ordinary schema migrations, then imports `knowledge/state.json`,
   `integrations/*.json`, `publications/*.json` and `receipts/*.json` in one DB
   transaction, keeping original IDs and Git bindings. It refuses unexpected
   registered Repos, unfinished owners and active Runs/integrations/reservations.
   No file is deleted or rewritten.
3. On error, keep the old files and fix the reported cause, then rerun the same
   command. Failed operational imports roll back; normal access refuses an empty
   DB fallback while legacy state exists. Schema migrations may already have
   applied: returning to the old binary requires restoring the pre-command DB
   backup, not simply swapping binaries. Old files remain available for that
   version. If the new version has written artifacts, restore the matching
   persistent snapshot as well; retain later data separately before recovery.
   Do not mix subsequent new-version writes with an old backup.
4. After success, start the new version through the authorised switch procedure.
   DB state is authoritative; repeating the command preserves it and does not
   reread stale file contents. Confirm normal Wiki/integration completion and
   recovery before separately authorising deletion of the four imported file
   groups. Also separately remove the retired Workspace Memory and dedicated
   verification groups; never delete the whole knowledge directory.

Keep Manifests (`events`), conversations, fixed `sync-inputs`, frozen
`source-publications`, source Run identity/completion checkpoints, and
setup/restoration journals with original instruction
bytes. They remain file artifacts or Git recovery inputs. Existing integration
validation evidence, reservations, remote MCP operations and Workspace results
remain in their existing DB models; import does not manufacture historical
execution bindings or treat Manifest test summaries as validation proof.

The importer, legacy decoders and unimported-state guard are temporary development
compatibility code. After the actual import and rollback window are complete,
remove that command/decoder/guard together. Keep the forward SQL schema migration
and ordinary DB model. Canonical OpenWiki must be updated separately through its
authorised maintenance lifecycle after integration.
