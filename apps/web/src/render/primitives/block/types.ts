// @pxl/web/render/primitives/block/types.ts

import type { f32, f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import type { V3 } from "@pxl/spec/space/v3.ts";

import type { OverlayColor, OverlayUniformState } from "@/render/overlay/types.ts";

export type BlockInstance = {
  base: V3;
  span: V3 | f64;
  color: OverlayColor;
};

export type BlockPass = {
  device: GPUDevice;

  pipeline: GPURenderPipeline;
  uniform: OverlayUniformState;

  instance_center_buf: GPUBuffer | null;
  instance_span_buf: GPUBuffer | null;
  instance_color_buf: GPUBuffer | null;

  instance_cnt: u32;
  instance_cap: u32;
};
