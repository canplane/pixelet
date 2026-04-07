// @pxl/web/render/anchor/types.ts

import type { FVec, IVec } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

export type AnchorLocalCoord = {
  anchor_idx: IVec;
  anchor_base: IVec;

  local_i: IVec;
  local_f: FVec;

  anchor_local_pos: V3;
};

export type AnchorEntry = {
  anchor_idx: IVec;
  local_base: V3;
};
