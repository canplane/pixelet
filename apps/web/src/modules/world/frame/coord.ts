// @pxl/web/modules/world/frame/coord.tsx

import { type IVec, ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L4_BASE } from "@pxl/spec/space/indexer.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

export function get_local_world_origin(map_center: PVF3 | null): IVec {
  if (!map_center) return 0n;
  return L4_BASE(map_center.i);
}

export function local_world_to_canonical(out: PVF3, local: Readonly<V3>, map_origin: IVec): PVF3 {
  projection.project(out, local[0], local[1], local[2]);
  out.i = ivec.add(out.i, map_origin);
  return out;
}
