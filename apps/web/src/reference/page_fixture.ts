// @pxl/web/reference/page_fixture.ts

/** A made-up terrain page in the core memory, for the tests that read one. */

import type { u32 } from "@xpute/core/kit/word.ts";
import { ELE_PADDED_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { type Ground, GROUND_CH_KEYS, GroundChannelKey, type Terrain } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { into_core } from "@/module/world/terrain/resident.ts";
import { GROUND_COLOR_LOOKUP_FLAT } from "@/render/gpu/layer_table.ts";

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

/** The type ids a channel's table knows, so a cell can carry a real one,
 * plus VOID; land also carries SEA, the bare type masked as water. */
function types_of(ch: GroundChannelKey): u32[] {
  const has = GROUND_COLOR_LOOKUP_FLAT[ch]?.has;
  const out: u32[] = [cube_types.common.VOID];
  if (has) { for (let t = 1; t < 256; t++) if (has[t]) out.push(t); }
  return out;
}

/** A page in the core memory: a rolling elevation raster with a few sharp
 * steps, and ground that is mostly land with water, structures and roads
 * sprinkled — enough of each that every channel decides some cell. */
export function page(seed: u32): Terrain {
  const next = rng(seed);
  const ele = new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN);
  for (let i = 0; i < ele.length; i++) ele[i] = -60 + (next() % 300) - (next() % 7 === 0 ? 200 : 0);
  const ground = {} as Ground;
  const types = Object.fromEntries(GROUND_CH_KEYS.map((k) => [k, types_of(k)])) as Record<GroundChannelKey, u32[]>;
  for (const key of GROUND_CH_KEYS) {
    const lane = new Uint8Array(PAGE_CELL_SPAN * PAGE_CELL_SPAN);
    const choices = types[key];
    const sparse = key !== GroundChannelKey.LAND;
    for (let i = 0; i < lane.length; i++) {
      if (sparse && next() % 5 !== 0) continue;
      lane[i] = key === GroundChannelKey.LAND && next() % 9 === 0 ? cube_types.land.SEA : choices[next() % choices.length];
    }
    ground[key] = lane;
  }
  return into_core({ ele, ground }, false);
}
