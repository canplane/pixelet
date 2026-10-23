# Decor gets the same lit/rim-light treatment as terrain and objects

**2026-09-06** — closes one of the two gaps left open by
[20260905-terrain-decor-scheduling-merged.md](./20260905-terrain-decor-scheduling-merged.md)
and its own tier-1 predecessor: those two passes unified _how_ terrain,
objects, and decor bake into geometry and _when_ they build (one step, one
`ready` flag), but never touched _what shader renders the result_ — decor
stayed on a plain unlit `MeshBasicMaterial` while terrain/objects got
`chunk_stage.tsx`'s own lit material with its fresnel rim highlight and
`light_cfg.FLOOR` night-floor treatment. `environment/config.ts`'s own
`rim_cfg` doc comment already states the actual governing principle — "a
shared render pipeline shouldn't treat one geometry role differently from
another for the same shader term" — decor not getting this was that
principle not having reached this file yet, not a considered exception.

## What changed

- **New `render/rim_light_shader.ts`**, factored out of `chunk_stage.tsx`'s
  own `inject_chunk_shader`: `inject_rim_light_shader(shader)` installs just
  the fresnel rim term (`u_rim_color`/`u_rim_intensity` uniforms, a
  `v_rim_view_pos` varying, the `pow(1-N·V, POWER)` falloff added to
  `outgoingLight`) — none of `inject_chunk_shader`'s own terrain-specific
  baked-attribute machinery (`a_rgb`/`a_layer_taps`/`a_visible`/
  `a_local_pos`/`a_box_scale`), which decor's plain glTF geometry
  (position/normal/uv only) has no use for. `chunk_stage.tsx` now calls it
  instead of inlining the same GLSL a second time; the two compose correctly
  regardless of call order (every replace target keeps its own marker text
  present in the substitution, the same chaining chunk_stage.tsx's remaining
  terrain-specific replacements already relied on).
- **`instanced_model.ts`'s decor material is now `MeshStandardMaterial`**,
  not `MeshBasicMaterial`: `roughness: 1, metalness: 0` (matching
  `chunk_stage.tsx`'s own `create_material()`), `emissive`/`emissiveMap`/
  `emissiveIntensity: light_cfg.FLOOR` (the same minimum-brightness floor
  every other lit surface gets — this covers the original 2026-09-05 "no
  guarantee of real scene lights bright enough" concern the unlit choice was
  solving, rather than reopening it), and `onBeforeCompile` wiring
  `inject_rim_light_shader`.
- **`hover_cursor.tsx`'s decor ghost preview follows**: switched from
  `meshBasicMaterial` to `meshStandardMaterial` with the same
  emissive-floor treatment, since the ghost's whole point (per its own doc
  comment) is "what a real render of this looks like, just translucent" — it
  has to track whatever the real thing does. It still skips the rim-light
  shader injection itself, same reasoning the pre-existing cube ghost
  already gives for skipping it: real coupling to a shader-injection path
  for a payoff a translucent preview doesn't need.

## A real constraint this hit: `deno test` and `*.glb?url`

The first pass wired the live `show_rim_light` setting straight into
`instanced_model.ts` (a `useSettingsStore` subscription next to the material
creation, mirroring `chunk_stage.tsx`'s own `ChunkMesh`). This broke
`deno task test` outright — not a logic bug, a module-graph one:
`app/settings/settings_store.ts` composes in
`slices/character_slice.ts`, which reaches `characters/variants.ts`'s own
`*.glb?url` asset imports. Vite handles that specifier; Deno's own test
loader has no transpiler for it, so any test file whose import graph reaches
it fails immediately with "Expected a JavaScript or TypeScript module, but
identified a Unknown module." `instanced_model.ts` is imported directly by
`decor_hit.test.ts` and `instanced_model.test.ts` — importing
`settings_store.ts` at that module's top level, even just for `.getState()`,
pulls the whole chain in regardless of which export is actually used.

Fixed by keeping the settings dependency out of `instanced_model.ts`
entirely: the material's own `u_rim_intensity` starts at 0 (same
"compiled in unconditionally, toggled via a uniform" pattern
`u_low_spec` already uses) and stays that way until something else sets it.
`decor_kinds.tsx`'s own `useAllDecorBundleAssets` — the one real (non-test)
consumer that needs the live value, and itself never imported by any test —
owns the `useSettingsStore` subscription instead, one level up, looping over
the (small, already-deduplicated-by-variant) materials array on every
`show_rim_light` toggle. `instanced_model.ts`'s own top doc comment now
records this constraint directly so a future edit doesn't reintroduce the
same import.

## Verification

`deno task check`/`lint`/`test` all green (139 tests, unchanged from the
prior pass — this touches no pure-function surface `decor_hit.test.ts`/
`instanced_model.test.ts` exercise, only material/shader setup; lint's
pre-existing 76-problem `xpute/core` baseline unchanged). Live: reloaded into
a populated scene, procedural trees render with visibly per-face lit
shading (each tree tier reads a distinct shade instead of one flat unlit
tone) and no shader-compile errors in the console (only the pre-existing,
unrelated `THREE.GLTFLoader: Couldn't load texture Textures/colormap.png`
noise); toggled the settings modal's rim light switch off and
back on and confirmed `show_rim_light` flips in the settings store as
expected. Did not re-verify the placement→hover→erase ghost round-trip live
this pass (the decor placement tool wasn't quickly reachable in this
session's UI pass) — the ghost's own material change is the same JSX shape
already proven correct for the pre-existing cube ghost in the same file, not
new code shape, so this is a lower-risk gap than the raw diff size suggests,
but still worth a manual look before calling this fully closed.
