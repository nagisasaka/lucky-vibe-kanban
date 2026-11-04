# Sidebar pets

Kanban and Workspace share one `SidebarPet` through `EnvironmentPanel`. It replaces only the clover placeholder; host observations and sidebar ownership are unchanged. One decorative pet is visible by default. Session activity never creates pets or controls their state. Clicking plays one waving cycle; no custom wink animation is assumed. The global “Host and pet” settings preview offers all nine states. The sidebar title and pencil open that section; there is no separate Pet disclosure row. Hidden tabs and reduced-motion preferences stop frame updates.

## Bundled artwork

LVK Rabbit v1.1.0 is pinned to commit `c8436fcc5ec04e806c1fb879d05e7c56254026ac`. The original manifest, atlas and full CC BY 4.0 text are in `packages/public/pets/lvk-rabbit/`. See [the attribution and material inspection record](../../packages/public/pets/lvk-rabbit/ATTRIBUTION.md) for the author **Masato Fujino**, source/release URLs, tag object, ZIP and file checksums, unchanged-artwork statement, publisher qualifications and exact inspection limits. The public release alone is not a quality acceptance test.

The settings retain visible source/version/license links. Artwork is unmodified. Imported artwork is never added to the LVK source tree or distributed as bundled content.

## Compatibility

The manifest is the existing Codex/Petdex `pet.json`, not a new pet format: `id`, `displayName`, `description`, `spritesheetPath`, and optional `spriteVersionNumber`. Omitted or `1` means v1: 1536×1872, 8 columns × 9 rows. Explicit `2` means v2: 1536×2288, 8 columns × 11 rows. Both use 192×208 cells and static PNG or WebP. The parsed version is retained in preferences, while old saved manifests without a version continue as v1. Unknown metadata is preserved in the original manifest for attribution. Unknown versions, custom state/frame declarations and scaled atlases are rejected rather than silently misplayed.

Primary reference: Petdex commit [31c0aee7f4fb387774b87d40e8047dc0f9cea94b](https://github.com/crafter-station/petdex/tree/31c0aee7f4fb387774b87d40e8047dc0f9cea94b), specifically `src/lib/sprite-atlas.ts`, `src/lib/sprite-version.ts`, `src/lib/pet-states.ts`, and `packages/petdex-desktop-native/src/sprite.zig`. [Public docs](https://petdex.dev/docs) describe local packages and a gallery. The installed official Pets sprite-sheet contract (`work-pets/0.1.6/references/sprite-sheet-contract.md`) establishes the v2 direction layout below; direction semantics are not inferred from third-party gallery playback.

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

v2 keeps rows 0–8 and their frame counts/timing unchanged. Rows 9 and 10 contain **static look poses**, not two extra animation loops:

| Row | Columns 0–7 (degrees) |
| --- | --- |
| 9 | 0, 22.5, 45, 67.5, 90, 112.5, 135, 157.5 |
| 10 | 180, 202.5, 225, 247.5, 270, 292.5, 315, 337.5 |

Angles are clockwise in screen coordinates: up=0, right=90, down=180, left=270. Zero is an upward pose, not neutral. While idle, a v2 pet looks toward the mouse relative to the displayed pet's center, selecting the nearest 22.5° pose. The center dead zone (20% of displayed size), pointer leaving the document, window blur, cancellation and non-mouse input restore ordinary idle. Touch does not track. Reduced motion and hidden tabs disable tracking and frame updates. Click-to-wave and non-idle preview states take priority; look poses are held still without an animation timer. v1 has no direction rows and continues normal playback. The pet remains 80px in both sidebar and settings, without new controls or extra rows of UI.

Rabbit's `running` art is typing; `running-right`/`running-left` are locomotion. Failed, waiting and review remain separate rows. Imported pets determine their own visual semantics.

## Import and persistence

Choose a local ZIP in Pet settings. `pet.json` may be at the root or inside one package folder. Only one manifest is allowed. The atlas path must be a relative archive path. Import never fetches manifest URLs or executes code. Validation completes before the existing attachments API stores the image and the existing UI preferences scratch stores selection, visibility, attachment references, original metadata and any root LICENSE/LICENCE/COPYING text. Preferences are global to the instance in the existing scratch mechanism, shared by both screens, rather than scoped to an Issue/Workspace/Session. Pet changes wait for acknowledgement from the existing scratch writer before changing selection or visibility. A missing/disconnected writer or failed save reports an error and retains the last selection. Uploaded images whose settings save fails remain unreferenced and follow normal attachment cleanup. A missing image falls back to Rabbit with Rabbit's accessible name and attribution; the retained selection's metadata is labelled separately. The orphan-file query and conditional delete both protect attachment IDs referenced by Pet preferences.

Limits: ZIP 8 MiB, 32 entries, 12 MiB declared total output, 8 MiB per file, manifest 16 KiB, retained license 64 KiB, at most 12 imported pets. ZIP output is bounded during inflation and CRC checked. Absolute/traversal/backslash/URL paths, duplicate case-insensitive names, symlinks, encrypted/multipart/ZIP64 archives and unsupported compression are rejected. Images must have an allowed actual PNG/WebP header and fixed dimensions before decoding; browser decoding must then succeed with dimensions matching the declared version. v2 additionally requires all 73 used cells to be non-empty and all 15 unused tail cells to be fully transparent, checked on import and restored-image load. Invalid/missing v2 image data falls back to Rabbit without replacing the retained selection or metadata. **v1 retains its historical dimension/decode-only acceptance**: painted tails or empty cells in previously usable v1 artwork do not newly cause rejection or fallback. Structural tests cannot establish the artistic meaning of a pose. PNG animation chunks (acTL/fcTL/fdAT) and WebP animation chunks (ANIM/ANMF) are rejected after bounded chunk scans, even when animation flags are misleading. Decode support and `DecompressionStream('deflate-raw')` depend on the browser; unavailable decoding fails safely. Stored ZIP entries are also supported.

Removing a pet removes its settings reference; attachment retention/cleanup follows the existing server lifecycle. Never delete a globally deduplicated attachment through the pet selector. Archive extras are validated but never extracted to a filesystem. Imported manifest/license text is rendered as text, with no executable HTML or imported hyperlinks.

## Integration with LOCAL-300

Both Kanban and Workspace render `SidebarPet` through the common `EnvironmentPanel`. Global Host and pet settings owns pet controls and instance display-name editing. v2 changes remain inside `features/pets/`; `UtilitySidebar`, Workspace `RightSidebar` and Kanban `ProjectUtilities` need no new controls.

## Verification boundary

Run the isolated component and real-sidebar composition checks described in [tests/pets/README.md](../../tests/pets/README.md). The latter uses the actual Kanban `ProjectUtilities`, Workspace `RightSidebar`, common environment panel, preference store and attachment client, with isolated HTTP responses and a registered local fixture writer. It covers shared selection/visibility, real Kanban Close handling, sidebar unmounts, 320/375/768/1000px widths and the global settings entry. It does not mount the full product router, board/layout/mobile sheet or contact a live backend. Browser fixture persistence is not proof of live scratch/attachment authorisation; Rust serde and retention tests cover their separate server boundaries.

V2 verification extends the existing isolated component browser test with generated geometric PNG/WebP atlases (test fixtures, not new pet artwork): all sixteen direction coordinates, static holds, interaction priority, reduced motion, touch/leave fallback, ZIP rejection, saved v1/v2 reload and switches, and English/Japanese narrow dark / desktop light layouts. The user’s in-progress v2 artwork is not part of those fixtures and is not verified by them. No third-party atlas copy or runtime evidence is bundled in the product commit.
