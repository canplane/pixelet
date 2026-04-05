// @pxl/web/modules/camera/config.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

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
    ROT_GAIN_AZIMUTH: -0.28 * (Math.PI / 180),
    ROT_GAIN_POLAR: 0.01,
    DOLLY_GAIN: 0.0022,
  },

  smooth: {
    ROT_DAMP: 10,
    POS_DAMP: 10,
    CAM_DAMP: 10,
  },
} as const;
