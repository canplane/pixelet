// @pxl/web/render/anchor/registry.ts

import type { FVec, IVec } from "@pxl/spec/space/pvf3/mod.ts";
import { fvec, ivec, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { FROM_L4_IDX, L4_BASE, L4_IDX } from "@pxl/spec/space/indexer.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

import type { AnchorEntry, AnchorLocalCoord } from "./types.ts";

export class AnchorRegistry {
  private map_origin: IVec = 0n;
  private entries = new Map<IVec, AnchorEntry>();

  set_map_origin(map_origin: IVec): void {
    if (this.map_origin === map_origin) return;

    this.map_origin = map_origin;
    this.entries.clear();
  }

  get_map_origin(): IVec {
    return this.map_origin;
  }

  clear(): void {
    this.entries.clear();
  }

  get_anchor_entry(anchor_idx: IVec): AnchorEntry {
    let entry = this.entries.get(anchor_idx);
    if (entry) return entry;

    const anchor_base: IVec = FROM_L4_IDX(anchor_idx);
    const local: IVec = ivec.sub(anchor_base, this.map_origin);

    entry = {
      anchor_idx,
      local_base: [
        ivec.to_real(ivec.X(local)),
        ivec.to_real(ivec.Y(local)),
        ivec.to_real(ivec.Z(local)),
      ],
    };
    this.entries.set(anchor_idx, entry);

    return entry;
  }

  get_anchor_local_base(anchor_idx: IVec): V3 {
    const entry = this.get_anchor_entry(anchor_idx);
    return [entry.local_base[0], entry.local_base[1], entry.local_base[2]];
  }

  lower_anchor_local_coord(i: IVec, f: FVec = 0n): AnchorLocalCoord {
    const anchor_idx: IVec = L4_IDX(i);
    const anchor_base: IVec = L4_BASE(i);

    const local_i: IVec = ivec.sub(i, anchor_base);
    const local_f: FVec = f;

    return {
      anchor_idx,
      anchor_base,
      local_i,
      local_f,
      anchor_local_pos: [
        pvf3.to_real(pvf3.X({ i: local_i, f: local_f })),
        pvf3.to_real(pvf3.Y({ i: local_i, f: local_f })),
        pvf3.to_real(pvf3.Z({ i: local_i, f: local_f })),
      ],
    };
  }

  lower_world_pos(i: IVec, f: FVec = 0n): V3 {
    const local_i: IVec = ivec.sub(i, this.map_origin);

    return [
      ivec.to_real(ivec.X(local_i)) + fvec.to_real(fvec.X(f)),
      ivec.to_real(ivec.Y(local_i)) + fvec.to_real(fvec.Y(f)),
      ivec.to_real(ivec.Z(local_i)) + fvec.to_real(fvec.Z(f)),
    ];
  }
}
