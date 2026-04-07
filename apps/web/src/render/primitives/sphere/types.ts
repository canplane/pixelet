// @pxl/web/render/primitives/sphere/types.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import type { V3 } from "@pxl/spec/space/v3.ts";

import type { OverlayColor, OverlayUniformState } from "@/render/overlay/types.ts";

export type SphereInstance = {
  center: V3;
  radius: f64;
  color: OverlayColor;
};

export type SpherePass = {
  device: GPUDevice;

  pipeline: GPURenderPipeline;
  uniform: OverlayUniformState;

  instance_center_radius_buf: GPUBuffer | null;
  instance_color_buf: GPUBuffer | null;

  instance_cnt: u32;
  instance_cap: u32;
};
