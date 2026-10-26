# Cozy texture pack: side-face asset audit, parked issues

**2026-08-30** — issues found while integrating the "cozy" MS2-inspired texture
pack (mew design, `pixelet_ms2_texture_manifest.json`) that were deliberately
parked rather than fixed in place, to be revisited in one batch pass. mew design
closed out SPORTS and HELIPAD the same day, alongside a general revision of
APRON/MEDICAL/COMMERCIAL/EDUCATION's side art — see "Fixed already" for the
resolution, kept here (not deleted) as the record of what was wrong and why.

**Status: complete 2026-09-17.**

---

## Why this exists

The pack assigns 13 land types their own dedicated side (cliff-face) texture
instead of falling back to their top texture. Three problems surfaced from just
eyeballing the assets against each other — see below — which is reason enough to
expect more once every remaining pairing gets looked at deliberately rather than
as a side effect of chasing a specific visual complaint.

## Fixed already (context, not open items)

**BARE/SPORTS side swap.** BARE's top is a vivid grass green, but its side
override pointed at sourceCreation #28 (bare exposed soil, no grass cap) — a
visible seam. #30 (grass-capped soil) had been assigned to SPORTS instead, only
because it was next in the old simplepixel override key list
(`BARE`/`GREEN_DEEP`/`GREEN_LIGHT`/`SPORTS`) that the manifest's side section
happened to inherit — never checked against SPORTS's own top texture (a lighter
mint/turf green, distinct from #30's darker grass). Corrected: #30 is now BARE's
side; #28 is unassigned rather than silently reused elsewhere. See
`terrain_images.ts`'s own `GROUND_LAYER_SIDE_OVERRIDES_COZY` comment and
`manifest.test.ts` for the regression coverage.

**SPORTS had no side texture, now does.** No sourceCreation in the original
manifest was a good visual match for SPORTS's own top (mint/turf green, distinct
from every other green in the pack), so it fell back to its own top texture on
its side faces (same fallback every non-overridden type gets) rather than force
a mismatched reuse. mew design generated a real dedicated SPORTS side asset
(sourceCreationId `d8d53fe2-...`, palette `#88E0BE` — matching
`land_sports.png`'s own paletteHex exactly) the same day; wired into
`GROUND_LAYER_SIDE_OVERRIDES_COZY` and covered by `manifest.test.ts`.

**HELIPAD's side texture didn't read as a terrain cross-section, now replaced.**
The original `side_helipad.png` looked like a sky/water horizon (pale-to-deep
blue gradient with a horizon line) — nothing like the "material stacked on soil"
cliff-face look every other side texture has, and its colors didn't match
`land_helipad.png`'s lavender/gray. mew design replaced it with a proper
layered-lavender cross-section (sourceCreationId `b848a057-...`); the new one
visually matches its top and the rest of the pack's side-art convention.

**APRON/MEDICAL/COMMERCIAL/EDUCATION were also revised** (not flagged as wrong
originally — mew design's own general audit pass caught issues in them
independently, including removing background sky/horizon/cloud elements and
stricter front-on-cross-section framing across the whole active side set). All
four re-checked visually after the swap: consistent color family with their own
top textures, no sky/horizon content, thin top rim as specified.

## Open

### Not yet individually audited

SAND, RESIDENTIAL, INDUSTRIAL, and PUBLIC were each spot-checked once against
their own top texture during the initial integration and looked like reasonable
material continuations (consistent color family, cliff-face styling) — but that
was one pass by one viewer, not the systematic side-by-side contact-sheet
comparison mew's own audit process calls for, and it's exactly the kind of
one-off check that missed BARE/SPORTS and HELIPAD the first time. Worth a real
pass, even though nothing about them has been flagged as wrong.

## Recommendation for a future full pass

A real contact sheet (every top texture next to its side texture, or its
fallback-to-top note if it has none) beats one-off spot checks — that's what
would have caught BARE/SPORTS and HELIPAD in one look instead of two separate
rounds.
