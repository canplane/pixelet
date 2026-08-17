// @pxl/web/modules/world/environment/config.ts

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { VIEW_DIST } from "@/modules/camera/config.ts";

// 2x, matching the camera's own distances (see VIEW_DIST). This is the
// expensive end of that chain: CHUNK_RENDER_DIST below derives from it, so
// the ground AREA that builds chunk geometry goes as its square — 2x here is
// 4x the area.
//
// That is knowingly the same fog value an iPhone 13 mini discarded the tab
// under. What is different is that RENDER_RADIUS is not coming with it this
// time (2, not 3), so mounted pages stay at 25 rather than 49. Whether the
// area alone was the trigger or whether it needed the page count too is the
// open question this setting is here to answer — if the reload returns, the
// area was enough on its own and this is the number to walk back.
const FOG_FAR = 900 * 2;

/** Render-distance and fog configuration. */
export const render_cfg = {
  // VIEW_DIST (camera/config.ts) is sized against terrain/config.ts's
  // RENDER_RADIUS — the actual real-world edge of mounted terrain — so
  // FAR_CLIP has real margin past where geometry stops existing instead of
  // an arbitrary multiple. The real fog gradient is FOG_NEAR/FOG_FAR below
  // (THREE's own linear fog, wired in via environment.tsx's <fog>) — it
  // already landed inside RENDER_RADIUS's reach even before this, so
  // ground fades into the fog color before it runs out, rather than the
  // old, much tighter LOAD_RADIUS-driven mount boundary cutting geometry
  // off before fog had a chance to hide it. The sky dome is fog-exempt
  // (environment.tsx's `fog={false}`), so the horizon still reads as a
  // full vista — only ground geometry fades.
  FAR_CLIP: VIEW_DIST * 1.5,

  FOG_NEAR: 320 * 2,
  FOG_FAR,

  // How far out chunk_stage.tsx actually mounts a chunk, in real
  // `u` units — independent of RENDER_RADIUS's own page-count granularity
  // (terrain/config.ts). RENDER_RADIUS only decides which *pages* are
  // worth building a chunk list for at all (a coarse, cheap pre-filter);
  // this decides which of *those* chunks actually become scene objects.
  // Needed because RENDER_RADIUS can only step in whole L3_RANGE
  // increments (1024u at the current L3_SHAMT) — nowhere near fine enough
  // to land close to FOG_FAR without either cutting geometry off
  // before fog hides it (RENDER_RADIUS=1, ~0u worst case) or badly
  // overshooting it (RENDER_RADIUS=2, up to ~2048u — a real, measured
  // mobile rendering-load regression: reported live stutter that wasn't
  // present at the old, much smaller SHAMT/radius combination traced back
  // to this exact overshoot, not chunk count or build bursts). Margin is
  // one chunk's own width so a chunk's far edge doesn't visibly clip
  // mid-fade right at the cutoff.
  CHUNK_RENDER_DIST: FOG_FAR + CHUNK_CELL_SPAN * U_PER_CELL,
} as const;

/**
 * Minimum-brightness floor shared by every real-lit ("real" meaning: an
 * actual MeshStandardMaterial driven by environment.tsx's scene lights, as
 * opposed to terrain's old always-bright custom shader) surface in the
 * scene — placed objects (block.tsx) and terrain/object cells
 * alike (modules/world/chunk_stage.tsx). One shared constant, not two
 * separate copies, specifically so the two can't drift apart again the way
 * they did before this existed (terrain used to be immune to scene-light
 * dimming by construction while objects weren't, then briefly the reverse
 * once terrain adopted real lighting too — see chunk_stage.tsx's own git
 * history for that whole back-and-forth). Tune this one number to retune
 * both surfaces' night floor at once.
 */
export const light_cfg = {
  // 0.82 -> 0.5 -> 0.3 -> 0.6. The walk down was chasing "does this read as
  // night", judged on a desktop display; 0.3 won that on a monitor and then
  // lost badly on a real device, where it was simply too dark to see the
  // world. That earlier pass had called even 0.5 too bright — a verdict
  // reached on the wrong screen, and now overshot in the other direction.
  //
  // Raising this alone flattens the scene, since it lifts every face by the
  // same amount and erases the shading that carries depth. It moved together
  // with the moon's directional light (environment.tsx, 0.18 -> 0.3), which
  // adds brightness that still differs face to face.
  FLOOR: 0.6,
} as const;

/**
 * Fresnel/rim highlight for placed objects only (chunk_stage.tsx's
 * OBJECT_BOX-based ChunkInstanceMesh, not the TERRAIN_BOX one) — a thin
 * bright edge on faces that graze the view direction, the classic
 * "toy block catching light at its silhouette" cue that helps placed
 * objects read as distinct from terrain cells even when the same block
 * palette overlaps both.
 */
export const rim_cfg = {
  COLOR: "#ffffff",
  // pow(1 - N.V, POWER) falloff — 2.5 keeps the highlight a thin band right
  // at the silhouette instead of washing across the whole visible face the
  // way a low power (close to 1) would.
  POWER: 2.5,
  // 0.35 -> 0.2 (2026-08-16): 0.35 was never actually visually validated —
  // it shipped blind under a since-fixed program-cache bug that silently
  // no-opped rim light entirely (see create_material's customProgramCacheKey
  // comment). First real look at it read as too strong for the "thin edge
  // cue" this is meant to be, not a wash across the face.
  INTENSITY: 0.2,
} as const;

/**
 * Real shadow-map tuning (environment.tsx's sun/moon DirectionalLights,
 * gated by render/settings_store.ts's show_shadow_map). Removed once
 * (2026-08-14, mobile-perf tradeoff — see git history on this constant's own
 * name) after being built, tuned, and optimized (manual shadow.needsUpdate
 * throttling instead of per-frame autoUpdate) across several commits;
 * restored 2026-08-15 as an opt-in setting instead of shipping on by
 * default.
 *
 * FRUSTUM_HALF_EXTENT started back at its original tuned value (96,
 * ~4 texels/cell on the sun's 2048-texel map at the original U_PER_CELL of
 * 0.25) when this was restored — but a live scene-graph trace (2026-08-15)
 * found actually-rendered chunk geometry extending out to ~940 world units
 * from the anchor the shadow frustum recenters on (matched render_cfg's own
 * FOG_FAR, 900 at the time), while a ±96 frustum only covered the inner
 * ~20% of that.
 * Settled on 200 (400-unit box, ~5.1 texels/cell) as the coverage/sharpness
 * middle ground — covers most on-screen near/mid-range geometry without
 * cratering texel density the way ±400 did.
 *
 * SUN_NORMAL_BIAS/MOON_NORMAL_BIAS are DERIVED from FRUSTUM_HALF_EXTENT
 * (via BIAS_TEXELS below) rather than separately-tuned constants — a real
 * bug this session (2026-08-15) was exactly two of these drifting apart:
 * normalBias is a world-unit push along the surface normal meant to be
 * "about one texel's worth," so a fixed 0.05 tuned for one frustum size
 * silently becomes wrong (too small → self-shadow acne, too big →
 * "peter-panning," the shadow visibly detaching from its own caster's
 * base) the moment FRUSTUM_HALF_EXTENT changes, since texel size
 * (frustum_span / map_size) moves with it. Confirmed live: at
 * FRUSTUM_HALF_EXTENT=400 the then-fixed 0.05 (only ~0.13 texels at that
 * size) produced a detached-looking shadow tens of units from its caster;
 * a texel-relative bias fixed the gross detachment, and BIAS_TEXELS was
 * then walked down further (0.25 → 0.1) against a placed test cube until
 * the shadow read as touching its own base rather than merely "closer" —
 * still with no visible acne on nearby terraced terrain at that value.
 * Keeping it as a ratio means retuning FRUSTUM_HALF_EXTENT again later
 * can't silently reintroduce this.
 *
 * Known limitation carried over from the original, not yet re-solved: the
 * frustum's own hard edge can read as a flat dark patch at its boundary
 * (PCF sampling clamps there instead of falling back to "unshadowed"). The
 * original fix faded the shadow contribution to zero before the edge inside
 * a custom terrain shader chunk; that shader no longer exists as its own
 * thing post-unification (chunk_stage.tsx's shared instance shader), so this
 * fade hasn't been ported yet.
 */
const FRUSTUM_HALF_EXTENT = 200;
const SUN_MAP_SIZE = 2048;
const MOON_MAP_SIZE = 1024;

// Walked down from 0.25 to 0.1 to 0.04 (2026-08-15) chasing a lingering
// gap between the shadow and its caster's own base, but a fixed
// normalBias's screen-space error grows as the light gets lower
// (~1/tan(elevation) worth of ground-plane shift for the same world-unit
// push), so the same 0.04 that looked fine on one caster read as a large,
// obviously detached gap on another once the sun sat lower — no per-object
// texel budget was ever going to paper over that. The actual fix was
// chunk_stage.tsx's create_material() setting material.shadowSide =
// THREE.BackSide: the shadow map now stores the caster's FAR-side depth
// instead of its own lit surface, so a receiver test no longer needs any
// push at all to dodge self-shadow acne. BIAS_TEXELS can sit at 0 as long
// as that shadowSide flip stays in place.
const BIAS_TEXELS = 0;

export const shadow_cfg = {
  FRUSTUM_HALF_EXTENT,
  SUN_MAP_SIZE,
  MOON_MAP_SIZE,
  SUN_CAMERA_FAR: 2400,
  MOON_CAMERA_FAR: 1200,
  SUN_BIAS: 0,
  SUN_NORMAL_BIAS: ((FRUSTUM_HALF_EXTENT * 2) / SUN_MAP_SIZE) * BIAS_TEXELS,
  MOON_BIAS: 0,
  MOON_NORMAL_BIAS: ((FRUSTUM_HALF_EXTENT * 2) / MOON_MAP_SIZE) * BIAS_TEXELS,
  // SHADOW_UPDATE_ANGLE_RAD's own reasoning (environment.tsx): re-rendering
  // a full 2048/1024-texel shadow map every frame for a light that only
  // drifts ~0.0042deg/s (a 24-real-hour sweep) is pure waste — only refresh
  // once the light direction has actually drifted past this angle since its
  // last render.
  UPDATE_ANGLE_DEG: 0.05,
} as const;

/**
 * Post-processing tuning for the "miniature diorama" look: a screen-space
 * contact-AO pass to ground blocky cells/buildings against each other, a
 * tilt-shift band that fakes a macro lens' shallow depth of field, and a
 * punchier color grade than the flat/raw scene output. All numbers here are
 * cosmetic dials — tune freely, nothing downstream depends on these values.
 */
export const post_cfg = {
  AO: {
    // halfRes + "performance" quality keeps this cheap enough to not add to
    // the existing close-zoom frame cost — see the LOD/chunk-density note in
    // terrain/chunk.tsx for why near-camera cost is already the tight budget.
    QUALITY: "performance",
    HALF_RES: true,
    // RADIUS: 6 was never tuned against real output — N8AO was broken on
    // the first attempt and got pulled before these numbers meant anything.
    // at 6 units, every small LOD-step riser sampled occlusion from several
    // neighboring risers up the same slope, outlining every real edge — on
    // a terraced hillside that read as contour-line banding, not soft
    // ambient shadow (confirmed by bypassing the shader's own AO/height-tint
    // and watching the same bands survive). Shrunk to ~1 cell-neighborhood
    // so a riser only shadows its own base; intensity dropped alongside it
    // so the narrower radius doesn't just concentrate the same darkening.
    RADIUS: 1.5,
    INTENSITY: 1.2,
    DISTANCE_FALLOFF: 1,
    // warm/dark tint instead of pure black — reads as a contact shadow
    // rather than a rendering artifact.
    COLOR: "#1a1006",
  },
  BLOOM: {
    INTENSITY: 0.6,
    LUMINANCE_THRESHOLD: 0.85,
    LUMINANCE_SMOOTHING: 0.2,
    MIPMAP_BLUR: true,
  },
  TILT_SHIFT: {
    // start/end are the two endpoints of a single line, not a band's edges —
    // blur ramps with each pixel's perpendicular distance from that line.
    // [0,0.5]->[1,0.5] is horizontal at mid-height, so the ramp direction is
    // vertical (sharp middle row, blur top/bottom). A near-vertical line (the
    // original, by accident) made the ramp near-horizontal, reading as the
    // whole frame out of focus since the sharp strip sat off-screen.
    START: [0, 0.5] as [number, number],
    END: [1, 0.5] as [number, number],
    // must point along the start/end line's perpendicular (vertical here) or
    // blur samples smear diagonally instead of matching the ramp direction
    DIRECTION: [0, 1] as [number, number],
    // 0.14/0.55 read as too subtle for the miniature-diorama look this is
    // meant to sell — bumped up for a more pronounced tilt-shift. The
    // shader's blur radius is an ease-in curve (smoothstep), not linear, so
    // it's near-zero right at the sharp line before accelerating — TAPER
    // stretches/shrinks that ease-in region (bigger = wider sharp band),
    // BLUR scales the ceiling. Shrinking TAPER alongside raising BLUR keeps
    // the sharp middle band from also growing, so the emphasis reads as
    // "blurrier edges" rather than "sharp region got bigger too" — still
    // tuned to keep a normal framing (street level, not a wide shot)
    // legible, just noticeably softer top/bottom than before. 0.22 -> 0.16
    // (2026-08-16): read as too strong once actually compared live; still
    // above the original 0.14 that was judged too subtle.
    BLUR: 0.16,
    TAPER: 0.42,
    SAMPLES: 6,
  },
  GRADE: {
    SATURATION: 0.18,
    CONTRAST: 0.08,
    BRIGHTNESS: -0.01,
  },
  VIGNETTE: {
    DARKNESS: 0.45,
    OFFSET: 0.3,
  },
  CHROMATIC_ABERRATION: {
    // small lens-fringing offset, not a heavy glitch-art displacement.
    // 0.0006 -> 0.0018 (2026-08-16): the original was sub-pixel at normal
    // resolution (under 1px of channel split on a 1920-wide frame) — raised
    // to a few texels' worth, still edge-only via RADIAL_MODULATION below,
    // not a uniform full-frame glitch-art displacement.
    OFFSET: [0.0018, 0.0018] as [number, number],
    // radial (edge-only) rather than uniform: a real lens' fringing grows
    // toward the frame edge and stays ~0 at center, which also keeps the
    // effect from muddying the sharp middle band tilt-shift leaves in focus.
    RADIAL_MODULATION: true,
    MODULATION_OFFSET: 0.15,
  },
  GOD_RAYS: {
    SAMPLES: 60,
    DENSITY: 0.96,
    DECAY: 0.9,
    WEIGHT: 0.4,
    EXPOSURE: 0.6,
    CLAMP_MAX: 1,
  },
} as const;
