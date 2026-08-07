// @pxl/web/modules/world/render/config.ts

import { VIEW_DIST } from "@/modules/camera/config.ts";

/** Render-distance and fog configuration. */
export const render_cfg = {
  FADE_START: VIEW_DIST * 0.75,
  FADE_END: VIEW_DIST,

  FAR_CLIP: VIEW_DIST * 1.5,

  // was VIEW_DIST*0.75 / VIEW_DIST (1536/2048u) — far enough that only the
  // outermost LOD hand-off (LOD4) sat inside the fog gradient; LOD2/LOD3
  // were in clear air with no dither-blend covering them (only LOD0<->1 and
  // LOD1<->2 blend, see lod.ts). Pulled in to straddle those two hand-offs
  // instead, trading view distance for hiding them in haze. The sky dome is
  // fog-exempt (environment.tsx's `fog={false}`), so the horizon still
  // reads as a full vista — only ground geometry fades sooner.
  FOG_NEAR: 320,
  FOG_FAR: 900,
} as const;

/**
 * Shared between environment.tsx (which actually sets the sun/moon shadow
 * cameras' left/right/top/bottom to ±this) and shader.ts (which fades the
 * terrain's shadow contribution to zero *before* reaching this radius) — a
 * single source of truth so the two can't drift out of sync. Origin-centered
 * because floating-origin keeps visibly-active terrain near true (0,0,0),
 * see world.tsx.
 *
 * The shadow frustum was shrunk from ±768 to fix texel/cell resolution
 * aliasing (a shadow-map texel wider than a terrain cell self-shadows
 * incorrectly — see the resolution-aliasing commit). That fix traded away
 * coverage: past this radius, PCF sampling near the frustum's hard edge
 * clamps to boundary texels instead of gracefully falling back to
 * "unshadowed", which read as a flat dark rectangle sitting right at the
 * boundary — worse than the aliasing it replaced. Fading the shadow's own
 * contribution out before the edge (SHADOW_FADE_START..SHADOW_FADE_END,
 * both comfortably inside FRUSTUM_HALF_EXTENT) hides that clamp seam instead
 * of trying to eliminate it at the source.
 */
export const shadow_cfg = {
  FRUSTUM_HALF_EXTENT: 96,
  FADE_START: 68,
  FADE_END: 88,
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
    // still read as too blurry at 0.22/0.4. The shader's blur radius is an
    // ease-in curve (smoothstep), not linear, so it's near-zero right at the
    // sharp line before accelerating — a bigger TAPER stretches that ease-in
    // region (wider sharp band), BLUR scales the ceiling. Both dialed down
    // so a normal framing (street level, not a wide shot) stays sharp.
    BLUR: 0.14,
    TAPER: 0.55,
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
} as const;
