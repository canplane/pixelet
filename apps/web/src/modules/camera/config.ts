// @pxl/web/modules/camera/config.ts

import type { f64 } from "@xpute/core/kit/word.ts";

/** Camera projection and interaction limits. */

/** Control knob for global view-dependent distances. */
export const VIEW_DIST: f64 = 2048;

/** Default tilt angle in degrees from the ground plane. */
export const DEFAULT_TILT_DEG: f64 = 30;

export const camera_cfg = {
  projection: {
    FOV: 50,
    NEAR: 0.1,
  },

  view: {
    DEFAULT_DIST: 128,
    DEFAULT_AZIMUTH: 0,
    /** 0 = straight down from above, PI / 2 = horizontal. */
    DEFAULT_POLAR: ((90 - DEFAULT_TILT_DEG) * Math.PI) / 180,

    MIN_DIST: 4,
    MAX_DIST: VIEW_DIST * 0.75,
    MIN_POLAR: 0.001,
    MAX_POLAR: Math.PI / 2.05,
  },

  input: {
    // ROT_GAIN_POLAR is tuned per-pixel at a viewport height of ROT_GAIN_REF_HEIGHT.
    // input.tsx normalizes by the *current* viewport height (size.height), so a given
    // drag distance relative to the screen tilts by the same amount on any screen
    // size — a fixed px/degree rate would make tilt feel much weaker on a small
    // (e.g. mobile) viewport, where the achievable drag range in raw pixels is smaller.
    // (Azimuth has no fixed gain — it tracks the cursor's angle around screen center
    // 1:1, arcball-style; see do_rotate() in input.tsx.)
    ROT_GAIN_POLAR: 0.012,
    ROT_GAIN_REF_HEIGHT: 1080,

    DOLLY_GAIN: 0.0022,
  },

  smooth: {
    ROT_DAMP: 10,
    POS_DAMP: 10,
    CAM_DAMP: 10,
  },
} as const;
