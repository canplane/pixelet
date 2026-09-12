// @pxl/web/modules/world/environment/config.ts

import type { f64 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { VIEW_DIST } from "@/modules/camera/config.ts";

/**
 * The one render-range knob, in world `u`: how far ground geometry exists,
 * and where fog finishes hiding its edge. Everything else about range is
 * derived from it — the chunks that mount, the pages fetched, the pages
 * evicted (terrain/config.ts).
 *
 * It replaces three independently chosen numbers: a fog distance in world
 * units and two page radii in whole pages. Because both page radii keyed on
 * the page the view centre stood in, the delivered range was not the stated
 * one — geometry ended anywhere between one and two pages out depending on
 * where in its page the camera stood, and crossing a boundary moved the
 * boundary by a whole page. See
 * discussions/20260912-what-to-render-when.md.
 *
 * Ranges are measured from the view centre, not from the camera, because
 * residency has to be invariant under rotation: at a wide zoom the camera
 * sits up to ~1023u from that centre, and following it would sweep pages in
 * and out of the cache on a spin. Which tier a chunk shows and which chunk is
 * built first are camera-relative and always were — both measure the chunk's
 * own 3D bounds centre against the camera, so content standing 100u above the
 * ground is ranked where it actually is.
 *
 * Anchoring the range at the centre is only sufficient because the camera
 * always looks at it, which keeps the visible wedge nearby. Measured over
 * every zoom, tilt, aspect ratio and pivot height the camera allows, visible
 * ground reaches at most ~1532u from the centre — the worst case being a
 * close zoom onto something standing above the terrain, where the camera
 * rises with it and sees almost a full fog radius in every direction.
 * CHUNK_RENDER_DIST is 1664u, two chunks past the range rather than one, so
 * the margin (132u) also covers the anchor's own chunk quantization (up to
 * ~90u). render/view_range.test.ts pins this, and widening the field of view
 * or letting the camera look away from its pivot breaks it there first.
 *
 * 1536u is chosen against what it costs to hold. Chunks mount two chunks past
 * it (CHUNK_RENDER_DIST, 1664u), which touches 21 pages at worst; the pages
 * actually held are the nearest 32 (~129MB decoded), against the 49 pages
 * (~197MB) that discarded the tab on an iPhone 13 mini. It is wider than the 1024u that was
 * really guaranteed before, and it is the same everywhere rather than only at
 * the middle of a page.
 */
const RENDER_DIST: f64 = 1536;

/** Render-distance and fog configuration. */
export const render_cfg = {
  // VIEW_DIST (camera/config.ts) is sized past CHUNK_RENDER_DIST below, the
  // real edge of mounted terrain, so FAR_CLIP keeps margin over where
  // geometry stops existing rather than being an arbitrary multiple. The fog
  // gradient is FOG_NEAR/FOG_FAR (THREE's own linear fog, wired in via
  // environment.tsx's <fog>), and it now ends exactly where geometry does, so
  // ground always fades into the fog colour instead of being cut off while
  // still visible. The sky dome is fog-exempt (environment.tsx's
  // `fog={false}`), so the horizon still reads as a full vista — only ground
  // geometry fades.
  FAR_CLIP: VIEW_DIST * 1.5,

  /** Haze starts here and is total at RENDER_DIST, where geometry stops.
   * The fraction is the one the previous pair had, so the near edge sits
   * where it did; what changes is that the far edge is now a distance
   * geometry actually reaches, so the fade completes instead of being cut. */
  FOG_NEAR: RENDER_DIST * 0.42,
  FOG_FAR: RENDER_DIST,

  RENDER_DIST,
  // How far out chunk_stage.tsx actually mounts a chunk. Which pages exist to
  // mount from is a count rather than a distance (terrain/config.ts's
  // RESIDENT_PAGES). Two chunks past the range: one so a chunk's far edge does not
  // visibly clip mid-fade at the cutoff, and one for the margin the invariant
  // above needs. 21 pages at worst (~85MB decoded).
  CHUNK_RENDER_DIST: RENDER_DIST + 2 * CHUNK_CELL_SPAN * U_PER_CELL,
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
  // Judged on a real device, not a desktop monitor — a darker floor that read
  // as convincing night on a monitor was simply too dark to see the world on
  // a phone screen.
  //
  // Raising this alone flattens the scene, since it lifts every face by the
  // same amount and erases the shading that carries depth. It moves together
  // with the moon's directional light (environment.tsx), which adds
  // brightness that still differs face to face.
  FLOOR: 0.6,
} as const;

/**
 * Fresnel/rim highlight, applied uniformly to every chunk_stage.tsx baked
 * chunk mesh — terrain and placed objects alike, merged into one mesh per
 * chunk (chunk_mesher.ts) — a thin bright edge on faces that graze the view
 * direction, the classic
 * "toy block catching light at its silhouette" cue. Originally object-only
 * (meant to help placed objects read as distinct from terrain cells sharing
 * the same block palette), but a shared render pipeline shouldn't treat one
 * geometry role differently from another for the same shader term, so it now
 * applies everywhere show_rim_light is on.
 */
export const rim_cfg = {
  COLOR: "#ffffff",
  // pow(1 - N.V, POWER) falloff. POWER doesn't change peak brightness (at
  // N.V=0, right at the silhouette, pow(1, POWER) is always 1 regardless of
  // the exponent) — it controls how wide the lit band reads, low values
  // spreading the same falloff into a softer, more gradual transition
  // instead of a hard narrow line.
  POWER: 1,
  // Linear scale on the falloff term above — this is the one that actually
  // controls peak brightness.
  INTENSITY: 0.1,
} as const;

/**
 * Real shadow-map tuning (environment.tsx's sun/moon DirectionalLights,
 * gated by app/settings/settings_store.ts's show_shadow_map). Opt-in rather than
 * on by default, since it was previously pulled once for a mobile-perf
 * tradeoff before coming back with manual shadow.needsUpdate throttling
 * instead of per-frame autoUpdate.
 *
 * FRUSTUM_HALF_EXTENT=200 (400-unit box, ~5.1 texels/cell on the sun's
 * 2048-texel map) is the coverage/sharpness middle ground: actually-rendered
 * chunk geometry extends out to ~940 world units from the anchor the shadow
 * frustum recenters on, and a narrower frustum only covers a small fraction
 * of that near/mid-range geometry, while a much wider one craters texel
 * density instead.
 *
 * SUN_NORMAL_BIAS/MOON_NORMAL_BIAS are DERIVED from FRUSTUM_HALF_EXTENT (via
 * BIAS_TEXELS below) rather than separately-tuned constants. normalBias is a
 * world-unit push along the surface normal meant to be "about one texel's
 * worth," so a fixed value tuned for one frustum size becomes wrong (too
 * small → self-shadow acne, too big → "peter-panning," the shadow visibly
 * detaching from its own caster's base) the moment FRUSTUM_HALF_EXTENT
 * changes, since texel size (frustum_span / map_size) moves with it. Keeping
 * it as a ratio means retuning FRUSTUM_HALF_EXTENT later can't silently
 * reintroduce that drift.
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

// A fixed normalBias's screen-space error grows as the light gets lower
// (~1/tan(elevation) worth of ground-plane shift for the same world-unit
// push), so a value that looks fine on one caster reads as an obviously
// detached gap on another once the sun sits lower — no per-object texel
// budget can paper over that. The actual fix is chunk_stage.tsx's
// create_material() setting material.shadowSide = THREE.BackSide: the
// shadow map stores the caster's FAR-side depth instead of its own lit
// surface, so a receiver test no longer needs any push at all to dodge
// self-shadow acne. BIAS_TEXELS can sit at 0 as long as that shadowSide flip
// stays in place.
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
 * Post-processing tuning: a screen-space contact-AO pass to ground blocky
 * cells/buildings against each other, bloom and god rays off the sun, and
 * a punchier color grade than the flat/raw scene output. All numbers here
 * are cosmetic dials — tune freely, nothing downstream depends on these
 * values.
 */
export const post_cfg = {
  /** The canvas draws at the device's pixel ratio clamped to this
   * (scene.tsx's dpr comment on why the cap, and why it is not a cost). */
  DPR: [1, 2] as [number, number],
  /** SMAA runs on a display whose dpr is under this; at the cap the
   * resolution does the job (scene.tsx's AutoSmaa). */
  SMAA_BELOW_DPR: 2,
  AO: {
    // halfRes + "performance" quality keeps this cheap enough to not add to
    // the existing close-zoom frame cost — see the LOD/chunk-density note in
    // terrain/chunk.tsx for why near-camera cost is already the tight budget.
    QUALITY: "performance",
    HALF_RES: true,
    // Kept to ~1 cell-neighborhood so a riser only shadows its own base. A
    // wider radius has every small LOD-step riser sampling occlusion from
    // several neighboring risers up the same slope, outlining every real
    // edge — on a terraced hillside that reads as contour-line banding, not
    // soft ambient shadow. INTENSITY is scaled down to match, so the
    // narrower radius doesn't just concentrate the same darkening.
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
  GRADE: {
    SATURATION: 0.18,
    CONTRAST: 0.08,
    BRIGHTNESS: -0.01,
  },
  VIGNETTE: {
    DARKNESS: 0.45,
    OFFSET: 0.3,
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
