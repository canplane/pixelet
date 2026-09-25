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

import type { I16Array, U16Array, U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { ground_pack } from "@/catalog/cell/cube_type.spec.ts";
import { encoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";

/**
 * Cells on an L3 page's side, and the elevation raster's own padded span —
 * pxl-spec's `PAGE_CELLS` (space/pyramid.rs) and `ELE_PADDED_SPAN`, a sample
 * every 32 metres plus two of padding on each side, which is what gives a
 * bicubic sample a 4×4 kernel at every edge texel (bake/constant.rs).
 */
export const PAGE_CELLS = 512;
export const ELE_PADDED_SPAN = 36;

/** A cell's four channels in one word, as pxl-spec's bake/pack.rs reads it:
 * each channel a dense index into its own table (`ground_pack`), which the
 * catalog generates beside the numbers it maps. */
export function pack_ground_cell(numbers: { land: u32; water: u32; structure: u32; transport: u32 }): u32 {
  const { channels, index, bits, shift } = ground_pack;
  let cell = 0;
  for (let f = 0; f < channels.length; f++) {
    const number = numbers[channels[f]];
    const mask = (1 << bits[f]) - 1;
    cell = (cell & ~(mask << shift[f])) | (index[f][number] << shift[f]);
  }
  return cell >>> 0;
}

/** `[ele, cells]` — the terrain page as the bake writes it and the client
 * reads it, sixteen bits a cell. */
export function encode_terrain(ele: I16Array, cells: U16Array): U8Array {
  return encoder.encode(new TreeView().branch((__) => __.i16_array(ele).u16_array(cells)));
}
