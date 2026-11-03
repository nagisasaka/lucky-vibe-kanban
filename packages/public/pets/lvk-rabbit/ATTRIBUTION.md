# LVK Rabbit v1.1.0

LVK Rabbit — Masato Fujino, licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/).

- Source: https://github.com/nagisasaka/lvk-rabbit
- Release: https://github.com/nagisasaka/lvk-rabbit/releases/tag/v1.1.0
- Annotated tag object: `2d7de142f32a23edfdab351f2b1c143d45714e5c`
- Commit: `c8436fcc5ec04e806c1fb879d05e7c56254026ac`
- Download: https://github.com/nagisasaka/lvk-rabbit/releases/download/v1.1.0/lvk-rabbit-v1.1.0-petdex.zip
- Download SHA-256: `490c7d4d47cd6dfb4af247d99d004156092104140e97724844fe5beae436f3fa`
- `pet.json` SHA-256: `7d9cf9d24941e7a1c13ff26c9d37295c7efa42f2dc355b97a7cb6720be168638`
- `spritesheet.webp` SHA-256: `ff98ec4e6a0e7a0bebd36269230826e1689d4eec71ef0348a697bc29f6bdb46f`
- `LICENSE` SHA-256: `9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411`
- License text: copied from `LICENSE` at the pinned source commit; the release ZIP itself contains only `pet.json` and `spritesheet.webp`.
- Changes: none to the original manifest, artwork or license. LVK adds this attribution and verification record.

The publisher describes the character as designed with assistance from ChatGPT and animated with Codex / Hatch Pet. It is an independent community character, not an official OpenAI character or endorsement. The publisher's license applies to rights the licensor can grant, including any applicable rights in AI-assisted material.

## Material inspection (2026-11-03)

The downloaded ZIP checksum and remote annotated tag / peeled commit were checked against the values above. The actual manifest specifies `id`, `displayName`, `description`, and the local `spritesheetPath` (`spritesheet.webp`). The original atlas is a single RGBA WebP, 1536 × 1872, with 192 × 208 cells in an 8 × 9 grid. Alpha spans 0–255. The occupied frame counts by row are 6, 8, 8, 4, 5, 8, 6, 6, 6 (57 total); the other 15 cells are fully transparent.

All 57 occupied cells were composited against white and dark backgrounds and visually inspected, with an enlarged idle frame inspected for edges. The cream fur, pink ear interiors, round glasses, mint hoodie and clover laptop remain consistent. No obvious clothing holes or red/green chroma-key fringes were seen. Failed shows a slumped pose, waiting shows a contemplative pose, review shows a distinct inspection pose, and running shows typing; these are visually independent states.

The source's six available animated previews (idle, jumping, failed, waiting, running, review) were decoded frame by frame to verify their frame counts and nonzero timings. This inspection is a material/frame review; it does not by itself certify animation timing in the LVK renderer or constitute a human approval of every animation in live playback. Renderer playback and theme checks must be recorded with the application verification results.

Imported third-party pets are user-provided material and are not covered by this attribution or automatically redistributed with LVK.

## Renderer acceptance check (2026-11-03)

`node tests/pets/browser.mjs` passed against the actual `PetSprite` and `SidebarPet` components in isolated Chromium with the bundled atlas. All nine states decoded, used their expected atlas row and advanced frames under real browser timers; the click reaction returned from waving to idle, and reduced-motion mode stopped frame progression. Light and dark screenshots of the rendered sidebar pet and nine-state gallery were opened and visually inspected. An initially observed adjacent-cell fringe was fixed by clipping the renderer to one cell; the repeated screenshots show clean boundaries, retained mint clothing, cream fur, pink ears, glasses and laptop in both themes.

The material is accepted for bundling based on the pinned provenance/license checks, complete atlas-frame inspection, rendered light/dark checks and automated animation behavior checks above. This is an engineering acceptance decision. Static screenshots plus timer/frame assertions do not claim a human watched uninterrupted playback, and this isolated fixture does not certify the full Kanban/Workspace layout or a live attachment backend. The fixture uses a separate loopback origin, mocked preferences/attachment transport, and temporary ZIPs; it does not touch existing user settings or shared LVK data.
