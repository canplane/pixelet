// @pxl/web/modules/camera/config.ts

import type { f64 } from "@xpute/core/kit/word.ts";

/** Camera projection and interaction limits. */

// Only feeds render_cfg's FAR_CLIP (camera far-clip plane) now — sized
// against terrain/config.ts's RENDER_RADIUS (currently 2 pages, one page
// = 1024u -> ~2048u real reach from the camera pivot) so that margin stays
// proportional instead of an arbitrary leftover multiple.
/** Control knob for global view-dependent distances.
 *
 * MAX_DIST, fog and RENDER_RADIUS are one chain rather than three knobs: the
 * camera sits MAX_DIST from its pivot, fog has to reach past that or full
 * zoom-out is a screen of solid fog, and geometry has to exist as far as fog
 * can see. Everything here scales by 2, which keeps the haze density at full
 * zoom-out (~33% at the pivot) rather than trading it for reach. What that
 * costs is in FOG_FAR's own comment: the third link in the chain,
 * RENDER_RADIUS, is deliberately NOT following. */
export const VIEW_DIST: f64 = 2048 * 2;

/** Default tilt angle in degrees from the ground plane. */
export const DEFAULT_TILT_DEG: f64 = 30;

export const camera_cfg = {
  projection: {
    FOV: 50,
    NEAR: 0.1,
  },

  view: {
    DEFAULT_DIST: 4 * 4,
    DEFAULT_AZIMUTH: 0,
    /** 0 = straight down from above, PI / 2 = horizontal. */
    DEFAULT_POLAR: ((90 - DEFAULT_TILT_DEG) * Math.PI) / 180,

    // picked against the scale bar's own reading, not as a raw distance
    // number with no felt meaning: at a 1080px viewport and Seoul's
    // latitude, total_meters ≈ 0.21 * dist (see scale_bar.tsx), giving
    // MIN_DIST ≈ 0.25m, MAX_DIST ≈ 200m — a felt-right target, not an exact
    // contract (both constants shift with real screen size/latitude, same
    // as the scale bar's own reading).
    MIN_DIST: 1 * 2,
    MAX_DIST: 512 * 2,
    MIN_POLAR: 0.001,
    MAX_POLAR: Math.PI / 2.05,
  },

  input: {
    // ROT_GAIN_POLAR is tuned per-pixel at ROT_GAIN_REF_HEIGHT; input.tsx
    // normalizes by the current viewport height, so a drag relative to the
    // screen tilts the same amount on any screen size (a fixed px/degree
    // rate would feel weaker on a small/mobile viewport). Azimuth has no
    // fixed gain — it tracks cursor angle around screen center 1:1
    // (arcball, see do_rotate() in input.tsx). Negative because dy is
    // positive moving down-screen, and dragging up (dy negative) is the
    // natural "tilt toward horizon" gesture.
    ROT_GAIN_POLAR: -0.012,
    ROT_GAIN_REF_HEIGHT: 1080,

    DOLLY_GAIN: 0.0022,

    // azimuth is derived from atan2, unstable at small radius (Δangle ≈
    // tangential_jitter / radius — a few px of jitter swings the angle
    // wildly up close). Below this radius, azimuth tracking freezes instead
    // of following it; see do_rotate()/update_pinch() in input.tsx.
    MIN_ROTATE_RADIUS_PX: 24,
    MIN_TWIST_DIST_PX: 24,

    // A rotate-drag used to derive azimuth and polar simultaneously on
    // every move event — a visually "straight down" drag still swept
    // azimuth unless it started exactly on the vertical centerline, most
    // visible once polar saturated (MIN/MAX_POLAR or the ground-collision
    // clamp) and the same drag read as a large, unrelated azimuth spin.
    // AXIS_LOCK_THRESHOLD_PX gates a one-time decision once total
    // displacement passes this radius: whichever axis dominates locks in
    // for the rest of the gesture (see do_rotate() in input.tsx). Smaller
    // than MIN_ROTATE_RADIUS_PX on purpose — this only needs enough
    // displacement to tell vertical from horizontal, not a stable angle reading.
    AXIS_LOCK_THRESHOLD_PX: 12,

    // did_drag used to flip true on ANY nonzero pointermove delta between
    // pointerdown and pointerup — real pointers (trackpad clicks, high-DPI
    // mice, touch) almost always report a pixel or two of jitter even on a
    // "stationary" click, so genuine taps (map recenter, object place/erase)
    // were silently getting swallowed as a zero-distance drag. Gate on total
    // displacement from pointerdown instead (see tap_start_x/y in
    // input.tsx) — small enough that an intentional drag still starts
    // panning immediately, large enough to absorb click jitter.
    TAP_DRAG_THRESHOLD_PX: 4,
  },

  smooth: {
    ROT_DAMP: 10,
    POS_DAMP: 10,
    CAM_DAMP: 10,
  },
} as const;
