// @pxl/web/modules/world/environment/config.ts

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { VIEW_DIST } from "@/modules/camera/config.ts";

const FOG_FAR = 900;

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

  FOG_NEAR: 320,
  FOG_FAR,

  // How far out chunk_stage.tsx actually mounts a chunk, in real
  // `u` units — independent of RENDER_RADIUS's own page-count granularity
  // (terrain/config.ts). RENDER_RADIUS only decides which *pages* are
  // worth building a chunk list for at all (a coarse, cheap pre-filter);
  // this decides which of *those* chunks actually become scene objects.
  // Needed because RENDER_RADIUS can only step in whole L3_RANGE
  // increments (1024u at the current L3_SHAMT) — nowhere near fine enough
  // to land close to FOG_FAR=900 without either cutting geometry off
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
  // 0.82 -> 0.5 -> 0.3: still trending down toward a real night floor.
  // 0.5 read as too bright to sell "night" at all; 0.3 keeps just enough
  // lift that a block's own hue is still legible in near-black shadow
  // instead of crushing to true black, which would make unlit geometry
  // indistinguishable from empty space (no depth cue left to read at all).
  FLOOR: 0.3,
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
    // legible, just noticeably softer top/bottom than before.
    BLUR: 0.22,
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
} as const;
