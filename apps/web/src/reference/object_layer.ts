// @pxl/web/reference/object_layer.ts

/**
 * Sparse placed objects -> shared temporary 64x64 chunk. The paint is the
 * module's (crates/pxl/core/src/world/object.rs's Painter) over the store's
 * records, which are the module's too; what this file does is teach it
 * what each state paints as, once, and hand it the drawn build's edits.
 */

import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { column_render_edits, object_column_of } from "@/module/world/object/store.ts";

import { type ChunkVolume, note_paint } from "@/reference/volume.ts";
import type { ChunkKey, WorldLayer } from "@/reference/contract.ts";
import { CORE_RING } from "@/wasm/core_pool.ts";
import { packet } from "@/wasm/core_ring.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { teach_objects } from "@/module/world/object/teach.ts";

export function chunk_origin_of(page_idx: IVec, page_off: IVec): IVec {
  return ivec.add(FROM_L3_IDX(page_idx), page_off);
}

// The tests run the reference against the module: the module has to know
// the kinds the reference paints.
teach_objects();

export class ObjectWorldLayer implements WorldLayer {
  readonly name = "objects";

  paint(key: ChunkKey, target: ChunkVolume, opts?: { for_raycast?: boolean }): boolean {
    const ring = CORE_RING;
    if (ring === null) throw new Error("render/object: the object layer paints through the module");
    const base = object_column_of(key.page_idx, key.page_off);
    // A raycast paints the real set; only the drawn build sees the eraser
    // preview's edits (store.ts's set_virtual_hide).
    const e = column_render_edits(base);
    const render = !opts?.for_raycast;
    const hide = render && e.hide !== 0;
    const carve = render && e.carve !== 0;
    const pkt = packet((p) =>
      p.u32(target.id).u64(BigInt.asUintN(64, base))
        .branch(hide ? (h) => h.i32(e.hide_band).u32(e.hide_pos) : null)
        .branch(carve ? (c) => c.f64(e.lx).f64(e.ly).f64(e.lz).f64(e.size) : null)
    );
    const made = ring.call(Cmd.OBJECT_PAINT, pkt, (bits) => bits);
    note_paint(target, (made & 2) === 0 ? 1 : 0);
    return (made & 1) !== 0;
  }
}
