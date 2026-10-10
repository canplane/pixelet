// script/vector/pxl/shared/spec/cell/mod.ts
//
// The cell catalogs as the TypeScript builds them — every channel's types
// and colours, the flat and decor ids, and every placeable id with its
// number, colour and source — for crates/pxl/shared/src/spec/cell/mod.test.rs.

import * as cell from "@pxl/shared/spec/cell/mod.ts";

export function vectors() {
  return {
    channels: Object.entries(cell.cube_types).map(([channel, types]) => ({
      channel,
      types: Object.entries(types).map(([name, num]) => `${name}=${num}`),
    })),
    colors: Object.entries(cell.cube_colors).map(([channel, colors]) => ({
      channel,
      colors: Object.entries(colors).map(([num, hex]) => `${num}=${hex ?? "null"}`),
    })),
    cube_ids: cell.cube_ids.map((id) => `${id}=${cell.cube_num_id(id)}=${cell.cube_hex(id)}`),
    decor_ids: cell.decor_ids.map((id) => `${id}=${cell.decor_num_id(id)}`),
    placeable: cell.placeable_cube_ids.map((id) => {
      const source = cell.is_terrain_style_cube(id) ? cell.terrain_style_cube_source(id) : null;
      return {
        id,
        num: cell.placeable_cube_num_id(id),
        hex: cell.placeable_cube_hex(id) ?? "null",
        back: cell.placeable_cube_id_from_num(cell.placeable_cube_num_id(id)),
        source: source ? `${source.channel}.${source.terrain_num_id}` : "none",
        is_decor: cell.object_value_is_decor(cell.placeable_cube_num_id(id)),
      };
    }),
    style_for: [["land", 12], ["water", 3], ["transport", 21], ["structure", 9]].map(([ch, n]) => cell.terrain_style_cube_for(ch as never, n as number) ?? "null"),
  };
}
