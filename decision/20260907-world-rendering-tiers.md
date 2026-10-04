# How the world's rendered things split into tiers, and the one billboard slot still missing

**2026-09-07** — prompted by a design question, not a code change: now that
terrain/objects/decor are unified into one baked-mesh pipeline (see
[20260903-terrain-decor-render-unification.md](./closed/20260903-terrain-decor-render-unification.md)
and [20260905-tier1-geometry-baking-implemented.md](./closed/20260905-tier1-geometry-baking-implemented.md)),
how should the _other_ things the world renders — HoverCursor, LocationPuck,
Companion (person/pet), chat bubbles, and a POI feature that was implemented
once (as of the 2026-01-09 area) and dropped before this project's current DoD
hardened — relate to that pipeline as more of them show up? No code changed
alongside this document; it's the reasoning to have on hand the next time one
of these grows.

## The five tiers that already exist, whether named or not

Surveying every non-terrain/object thing currently rendered turned up five
genuinely different rendering patterns, each earning its shape from a real
technical constraint — not five arbitrary choices:

| Tier                                    | Examples                          | Mechanism                                                                                                                                                                                                                                                                                                                                                                         | Why this shape                                                                                                                                                                                                                    |
| --------------------------------------- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tier                                    | Examples                          | Mechanism                                                                                                                                                                                                                                                                                                                                                                         | Why this shape                                                                                                                                                                                                                    |
| -------------------------------------   | --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                     | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0. Voxel layers                         | Terrain, Objects (`WorldLayer`)   | Paint into a shared per-chunk dense volume; one merged mesh per chunk                                                                                                                                                                                                                                                                                                             | Homogeneous, unboundedly many, occupies real volume other layers must occlude/merge against                                                                                                                                       |
| 1. Baked-instance layers ("Instance")   | Decor                             | Real per-instance vertex data merged into one `BufferGeometry` per (chunk, variant) — despite the `instanced_model.ts` filename, this is _not_ `THREE.InstancedMesh`                                                                                                                                                                                                              | Many repeated small static things; same "one draw call" goal as tier 0, but never needs to reason about occlusion with tier 0's own voxels, so it's a sibling pipeline rather than a `WorldLayer` implementation                  |
| 2. Actors ("Actor")                     | Companion (person + pet)          | A full cloned GLTF scene graph per instance, position/animation driven imperatively in `useFrame` via refs (not React state/props, so 60fps `map_center` churn never forces a re-render)                                                                                                                                                                                          | Few, individually distinct, genuinely animated — `instanced_model.ts`'s own doc comment names this split explicitly: fine to clone full scene graphs at "one companion" scale, wrong at "hundreds of scattered instances" scale   |
| 3. World-anchored billboards ("Bubble") | ChatBubble, POI                   | Canvas-drawn content baked to a `CanvasTexture`, rendered as a `THREE.Sprite` (camera-facing for free) via billboard/billboard.tsx's shared `<Billboard>`, `depthTest={false}` + high `renderOrder` so it reads as UI, not real occludable geometry — a constraint of the Sprite, not a preference, see [20260909-lod-look-and-seams.md](./closed/20260909-lod-look-and-seams.md) | Informational overlay content that must face the camera regardless of world rotation                                                                                                                                              |
| 4. One-off indicators                   | HoverCursor, LocationPuck         | Bespoke geometry/material, anchored via `AnchorNode`, **not** camera-facing (LocationPuck's beacon spins/bobs in world space; its zone ring is a real terrain-following instanced mesh)                                                                                                                                                                                           | Singular, ephemeral, tied to input/device state rather than persisted world data                                                                                                                                                  |

Bubble/Actor/Instance above are informal names for tiers 3/2/1 — nothing
physically unifies same-tier things into one module (there's no `BubbleLayer`
managing POI+ChatBubble together, for instance), just a shared vocabulary for
talking about them.

Tiers 3 and 4 look similar at a glance (both are "one bespoke mesh anchored
in the world") but differ on the one trait that actually matters here:
whether the content must counter-rotate to face the camera. That's the trait
POI and chat bubbles share and HoverCursor/LocationPuck don't.

## The gap: no reusable world-anchored billboard

[20260811-coordinate-system.md](./closed/20260811-coordinate-system.md) already
named this gap from the camera-frame side: of the four theoretical
translation/rotation-variance combinations, three have a real slot and one —
translation-variant, rotation-invariant, i.e. "stays at a world position but
always faces the camera" — has none. That document's own example of the
missing case was POI labels.

ChatBubble is close, but it doesn't actually fill that slot: it works by being
a child of `Companion`'s own `<primitive>` (inheriting the character's
transform) and letting `THREE.Sprite`'s material do the counter-rotation —
it never touches `AnchorNode` and has no standalone world position of its
own. There is currently no component that takes a bare world `i`/`f` and
renders camera-facing content there.

## Recommendation: don't generalize into one Entity/Layer abstraction

The four non-voxel tiers above have real, different constraints — whether
they occlude real geometry, whether they can be baked/merged, whether they
need per-frame imperative updates, how many of them exist at once. Unifying
them under one `Entity` or generalized `WorldLayer` interface (the framing an
LLM suggested when this came up) would average away exactly the differences
that make each tier's current implementation the right one for its own job,
in exchange for an abstraction none of the four tiers would actually use in
full.

The concrete, useful move instead: name the tiers (done, above, so future
work has a place to look) and fill the one real gap — extract ChatBubble's
canvas-texture-to-sprite logic (notched-bubble drawing aside) into a shared
`AnchorBillboard`-style primitive next to `anchor/node.tsx`, the first time a
second tier-3 consumer (POI) actually needs it. Not before — there's only one
consumer today, and generalizing a pattern that has never been reused yet is
the same premature-abstraction mistake in miniature.

## If/when POI comes back

Two sub-questions came up alongside this one, both already answered enough to
record:

- **Label rendering should be canvas/sprite (tier 3), not the old HTML-portal
  approach.** The prior implementation's "severe overhead" matches the known
  failure mode of DOM-follows-3D-object markers: a screen-space reprojection
  and a `style.transform` write per marker per frame, entirely outside the
  WebGL render loop, that stops batching the moment enough markers are on
  screen at once (a city viewed zoomed-out easily has dozens). A sprite is a
  GPU draw call that rides the existing render loop for free via `AnchorNode`,
  and can reuse decor's already-built LOD-thinning machinery
  ([20260906-decor-lod-blend.md](./closed/20260906-decor-lod-blend.md)'s shared
  visibility registry, or a simpler cut against `poi_extractor.ts`'s existing
  `rank` field) to declutter at low zoom instead of reinventing that per
  marker. A click-triggered detail panel (the old `poi-detail-panel.tsx`) is
  a different shape — one at a time, no per-frame position tracking pressure
  — and can stay ordinary DOM/React UI like the rest of this app's overlays
  (`settings_modal.tsx`, `top_overlays.tsx`) without inheriting the label
  layer's constraints.
- **The reverse-geocoding provider question (Naver/Kakao/Nominatim
  swappability) is a data-layer concern, decoupled from the above.** Whatever
  produces a POI record — bake-time OSM extraction (`poi_extractor.ts`,
  still live today) or a live reverse-geocode call through a provider — both
  converge on the same "world-anchored, labeled point" shape the render side
  consumes, so the render design above doesn't need to know which one is
  behind it. [20260816-deployment-hosting.md](./20260816-deployment-hosting.md)
  already documents `VITE_GEOCODE_BASE_URL` as a deliberately-unexercised
  seam for this ("a door left open, not one to walk through now"); a small
  `GeocodeProvider`-shaped interface (`reverse(coord) => {label, category} |
  null`) is worth designing on its own terms when POI work actually resumes,
  not as a prerequisite for the render-side decision above.

## Follow-up (2026-09-09): tier 3 has no real DOM interaction, and doesn't need one per instance

POI shipped since this document was first written
([20260908-overture-poi-adoption.md](./20260908-overture-poi-adoption.md)),
making the question concrete: can a tier-3 billboard support hover-title
tooltips (POI) or drag-to-select text (ChatBubble)? Checked against the
actual code (`sprite_billboard.tsx`, `chat_bubble.tsx`) — no. A billboard's
content is `ctx.fillText()` baked into a `CanvasTexture` on a `THREE.Sprite`;
there is no DOM text node for the browser to select, and no element for a
native `title` attribute to hang off. This isn't a bug, it's what tier 3 is
— see the "if/when POI comes back" note above ruling out a per-marker HTML
overlay by name, for the exact reason DOM-follows-3D-object markers don't
scale.

That reasoning still holds; it constrains the answer rather than closing the
question. Figma solved the same problem at a much larger scale with a
pattern worth borrowing: the canvas stays the only thing ever drawn, and a
**single hidden native input, positioned over whatever is currently
active**, captures real browser behavior (text selection, drag, IME
composition) exactly when something is actually being interacted with —
never one gizmo held open per shape. Applied here: one shared tooltip DOM
node (not one per POI) repositioned to a raycast hit's screen-projected
point on hover, and for ChatBubble, only the currently-visible bubble's text
region gets a real (invisible-until-dragged) text overlay — the notched
bubble shape stays canvas-drawn as it is today. The wiring both would share:
reuse `hover_cursor.tsx`'s existing raycast-hit-test, project the hit to
screen space (`camera.project()`), and drive a plain DOM element's
`style.transform` imperatively — the same ref-mutation-over-`useFrame`
convention `use_distance_fade.ts` already uses, not React state per frame.

Idea only, nothing built. Worth doing the first time either need is real,
not before — same premature-abstraction caution as the rest of this
document.

## Follow-up (2026-09-09): the reverse-geocoding data-layer question, resolved

`poi_extractor.ts` is gone — POI comes from
`apps/api/src/bake/poi/providers/overture_provider.ts` (Overture Places).
Reverse geocoding shipped its own `GeocodeProvider` (`apps/api/src/bake/geocode/provider.ts`,
`providers/overture_provider.ts`) against Overture's Divisions theme, not
Naver/Kakao/Nominatim. `VITE_GEOCODE_BASE_URL` no longer exists.

## Status

Discussion only — no code changed. Nothing here supersedes an earlier
decision; it names a pattern that was already implicit in the code and
records an open gap for later.
