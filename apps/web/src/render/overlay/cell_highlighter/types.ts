// @pxl/web/render/overlay/cell_highlighter/types.ts

import type { f32, f64 } from "@xpute/spec/lib/kit/word.ts";

import type { V3 } from "@pxl/spec/space/v3.ts";

export type CellHighlighterInstance = {
  pos: V3;
  scale: f64;
  color: [f32, f32, f32, f32];
};
