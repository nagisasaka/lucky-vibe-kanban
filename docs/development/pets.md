# Sidebar pets

Kanban and Workspace share one `SidebarPet` through `EnvironmentPanel`. It replaces only the clover placeholder; host observations and sidebar ownership are unchanged. One decorative pet is visible by default. Session activity never creates pets or controls their state. Clicking plays one waving cycle; no custom wink animation is assumed. The settings preview offers all nine states and returns to idle when closed. Hidden tabs and reduced-motion preferences stop frame updates.

## Bundled artwork

LVK Rabbit v1.1.0 is pinned to commit `c8436fcc5ec04e806c1fb879d05e7c56254026ac`. The original manifest, atlas and full CC BY 4.0 text are in `packages/public/pets/lvk-rabbit/`. See [the attribution and material inspection record](../../packages/public/pets/lvk-rabbit/ATTRIBUTION.md) for the author **Masato Fujino**, source/release URLs, tag object, ZIP and file checksums, unchanged-artwork statement, publisher qualifications and exact inspection limits. The public release alone is not a quality acceptance test.

The settings retain visible source/version/license links. Artwork is unmodified. Imported artwork is never added to the LVK source tree or distributed as bundled content.

## Compatibility

The manifest is the existing Codex/Petdex `pet.json`, not a new pet format: `id`, `displayName`, `description`, `spritesheetPath`, optionally `spriteVersionNumber: 1`. Unknown metadata is preserved in the original manifest for attribution. The renderer supports fixed v1 atlases: 1536×1872, 8 columns × 9 rows, 192×208 cells, PNG or WebP. Custom state/frame declarations are rejected rather than silently misplayed. Scaled atlases and all v2 packages are currently unsupported.

Primary reference: Petdex commit [31c0aee7f4fb387774b87d40e8047dc0f9cea94b](https://github.com/crafter-station/petdex/tree/31c0aee7f4fb387774b87d40e8047dc0f9cea94b), specifically `src/lib/sprite-atlas.ts`, `src/lib/sprite-version.ts`, `src/lib/pet-states.ts`, and `packages/petdex-desktop-native/src/sprite.zig`. [Public docs](https://petdex.dev/docs) describe local packages and a gallery, but do not establish complete v2 runtime compatibility. Upstream v2 is an 8×11 atlas; its extra directions/rows have not been validated in LVK and are deliberately rejected.

Playback follows the native frame hold durations, rather than distributing a cycle evenly as the upstream web gallery does:

| Row | State | Frames | Frame durations (ms) |
| --- | --- | --- | --- |
| 0 | idle | 6 | 280,110,110,140,140,320 |
| 1 | running-right | 8 | 120,120,120,120,120,120,120,220 |
| 2 | running-left | 8 | 120,120,120,120,120,120,120,220 |
| 3 | waving | 4 | 140,140,140,280 |
| 4 | jumping | 5 | 140,140,140,140,280 |
| 5 | failed | 8 | 140,140,140,140,140,140,140,240 |
| 6 | waiting | 6 | 150,150,150,150,150,260 |
| 7 | running | 6 | 120,120,120,120,120,220 |
| 8 | review | 6 | 150,150,150,150,150,280 |

Rabbit's `running` art is typing; `running-right`/`running-left` are locomotion. Failed, waiting and review remain separate rows. Imported pets determine their own visual semantics.

## Import and persistence

Choose a local ZIP in Pet settings. `pet.json` may be at the root or inside one package folder. Only one manifest is allowed. The atlas path must be a relative archive path. Import never fetches manifest URLs or executes code. Validation completes before the existing attachments API stores the image and the existing UI preferences scratch stores selection, visibility, attachment references, original metadata and any root LICENSE/LICENCE/COPYING text. Preferences are global to the instance in the existing scratch mechanism, shared by both screens, rather than scoped to an Issue/Workspace/Session. Pet changes wait for acknowledgement from the existing scratch writer before changing selection or visibility. A missing/disconnected writer or failed save reports an error and retains the last selection. Uploaded images whose settings save fails remain unreferenced and follow normal attachment cleanup. A missing image falls back to Rabbit with Rabbit's accessible name and attribution; the retained selection's metadata is labelled separately. The orphan-file query and conditional delete both protect attachment IDs referenced by Pet preferences.

Limits: ZIP 8 MiB, 32 entries, 12 MiB declared total output, 8 MiB per file, manifest 16 KiB, retained license 64 KiB, at most 12 imported pets. ZIP output is bounded during inflation and CRC checked. Absolute/traversal/backslash/URL paths, duplicate case-insensitive names, symlinks, encrypted/multipart/ZIP64 archives and unsupported compression are rejected. Images must have an allowed actual PNG/WebP header and fixed dimensions before decoding; browser decoding must then succeed with matching dimensions. PNG animation chunks (acTL/fcTL/fdAT) and WebP animation chunks (ANIM/ANMF) are rejected after bounded chunk scans, even when animation flags are misleading. Decode support and `DecompressionStream('deflate-raw')` depend on the browser; unavailable decoding fails safely. Stored ZIP entries are also supported.

Removing a pet removes its settings reference; attachment retention/cleanup follows the existing server lifecycle. Never delete a globally deduplicated attachment through the pet selector. Archive extras are validated but never extracted to a filesystem. Imported manifest/license text is rendered as text, with no executable HTML or imported hyperlinks.

## Integration with LOCAL-300

The only panel connection is the `SidebarPet` import and the replacement of the former `data-pet-placeholder` block in `packages/web-core/src/shared/components/EnvironmentPanel.tsx`; the section also establishes a relative position for the settings popover. LOCAL-300 may move this component within its top section without touching the independent `features/pets/` implementation. `UtilitySidebar`, Workspace `RightSidebar` and Kanban `ProjectUtilities` are unchanged. No LOCAL-300 workspace/branch is modified.

## Verification boundary

Run the isolated component and real-sidebar composition checks described in [tests/pets/README.md](../../tests/pets/README.md). The latter uses the actual Kanban `ProjectUtilities`, Workspace `RightSidebar`, common environment panel, preference store and attachment client, with isolated HTTP responses and a registered local fixture writer. It covers shared selection/visibility, real Kanban Close handling, sidebar unmounts, 320/375/768/1000px widths and narrow settings popovers. It does not mount the full product router, board/layout/mobile sheet or contact a live backend. Browser fixture persistence is not proof of live scratch/attachment authorisation; Rust serde and retention tests cover their separate server boundaries.
