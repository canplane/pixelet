// script/vector/pxl/shared/spec/cell/mod.ts
//
// The cell catalogs as the TypeScript builds them — every channel's types
// and colours, the flat and decor ids, and every placeable id with its
// number, colour and source — for crates/pxl/spec/src/cell/mod.test.rs.

import { GroundChannelKey } from "@pxl/shared/bake/format.ts";
import * as cell from "@pxl/shared/spec/cell/mod.ts";

export function vectors() {
  return {
    channels: Object.entries(cell.cube_types).map(([channel, types]) => ({
      channel,
      types: Object.entries(types).map(([name, num]) => `${name}=${num}`),
    })),
    colors: Object.entries(cell.cube_colors).map(([channel, colors]) => ({
      channel,
      colors: Object.entries(colors).map(([num, swatch]) => `${num}=${swatch?.hex ?? "null"}`),
    })),
    cube_ids: cell.cube_ids.map((id) => `${id}=${cell.cube_num_id(id)}=${cell.cube_hex(id)}`),
    decor_ids: cell.decor_ids.map((id) => `${id}=${cell.decor_num_id(id)}`),
    placeable: cell.placeable_cubes.map((cube) => {
      const source = cell.terrain_style_cube_source(cube);
      const num = cell.placeable_cube_num_id(cube);
      return {
        id: cell.placeable_cube_id(cube),
        num,
        hex: cell.placeable_cube_hex(cube) ?? "null",
        back: cell.placeable_cube_id(cell.placeable_cube_of(num)!),
        source: source ? `${source.channel}.${source.terrain_num_id}` : "none",
        is_decor: cell.object_value_is_decor(num),
      };
    }),
    style_for: ([[GroundChannelKey.LAND, 12], [GroundChannelKey.WATER, 3], [GroundChannelKey.TRANSPORT, 21], [GroundChannelKey.STRUCTURE, 9]] as const)
      .map(([ch, n]) => {
        const cube = cell.terrain_style_cube_for(ch, n);
        return cube ? cell.placeable_cube_id(cube) : "null";
      }),
  };
}
