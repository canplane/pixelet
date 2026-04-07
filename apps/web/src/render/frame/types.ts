// @pxl/web/render/frame/types.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import type { RenderCameraState, RenderWorldState } from "../types.ts";

export type TickArgs = {
  width: u32;
  height: u32;
  dt: f64;
};

export type TickOut = {
  camera: RenderCameraState;
  world: RenderWorldState;
};
