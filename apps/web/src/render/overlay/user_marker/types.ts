// @pxl/web/render/overlay/user_marker/types.ts

import type { f32, f64 } from "@xpute/spec/lib/kit/word.ts";

import type { V3 } from "@pxl/spec/space/v3.ts";

export type UserMarkerBeaconInstance = {
  pos: V3;
  scale: f64;
  color: [f32, f32, f32, f32];
};

export type UserMarkerZoneInstance = {
  pos: V3;
  scale: f64;
  color: [f32, f32, f32, f32];
  borders: [f32, f32, f32, f32];
};

export type ZoneSample = {
  off: V3;
  borders: [f64, f64, f64, f64];
};
