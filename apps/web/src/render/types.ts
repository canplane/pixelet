// @pxl/web/render/types.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { F32Array } from "@xpute/spec/lib/kit/array.ts";

import type { IVec } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

export type RenderSurface = {
  canvas: HTMLCanvasElement;
  context: GPUCanvasContext;
  format: GPUTextureFormat;
  width: u32;
  height: u32;
};

export type RenderFrameArgs = {
  device: GPUDevice;
  surface: RenderSurface;
  time: f64;
  dt: f64;
};

export type RenderFrameFn = (args: RenderFrameArgs) => void;

export type RenderCameraState = {
  view_proj: F32Array;
  eye: V3;
  target: V3;
  up: V3;
  aspect: f64;
  fov_y_rad: f64;
  near: f64;
  far: f64;
};

export type RenderWorldState = {
  world_azimuth: f64;
  map_origin: IVec;
  local_world_offset: V3;
};
