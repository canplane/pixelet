// @pxl/web/kernel/fixture.ts

/**
 * The pages kernel.test.ts answers the kernel's fetches with.
 *
 * A page's wire format is `pxl-spec`'s — `bake/format.rs` writes what a bake
 * publishes and the kernel reads. This writes the same two shapes in
 * TypeScript, because the test drives the module from outside it and has
 * nothing else to hand it bytes.
 *
 * Nothing here is checked against the Rust, and nothing needs to be: a page
 * the kernel cannot decode is a page it does not draw, so a shape that drifts
 * takes `max_draws > 0` and the map's `success` down with it. The test fails
 * loudly rather than silently passing on a fixture nobody reads.
 */

import type { I16Array, U8Array } from "@xpute/core/abi/array.ts";
import { encoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";

/**
 * One L3 page's span in raster cells, and the elevation raster's own padded
 * span — `1 << (L3_SHAMT - CELL_SHAMT)` and `1 << (L3_SHAMT + 1 - 5)` plus two
 * cells of padding on each side, which is what gives a bicubic sample a 4×4
 * kernel at every edge texel (pxl-spec/bake/constant.rs).
 */
export const PAGE_CELL_SPAN = 1024;
export const ELE_PADDED_SPAN = 68;

/** `[ele, [land, water, structure, transport]]`. */
export function encode_terrain(ele: I16Array, ground: { land: U8Array; water: U8Array; structure: U8Array; transport: U8Array }): U8Array {
  return encoder.encode(
    new TreeView().branch((__) => __.i16_array(ele).branch((__) => __.u8_array(ground.land).u8_array(ground.water).u8_array(ground.structure).u8_array(ground.transport))),
  );
}

/**
 * A page the bake found nothing on: six lanes of length zero, then the four
 * string branches empty. What a client holds for a page with no POI, which is
 * every page this test asks for.
 */
export function encode_empty_poi_page(): U8Array {
  return encoder.encode(
    new TreeView().branch((__) =>
      __.f64_array(new Float64Array(0))
        .f64_array(new Float64Array(0))
        .f64_array(new Float64Array(0))
        .u16_array(new Uint16Array(0))
        .u16_array(new Uint16Array(0))
        .u64_array(new BigUint64Array(0))
        .branch(() => {})
        .branch(() => {})
        .branch(() => {})
        .branch(() => {})
    ),
  );
}
