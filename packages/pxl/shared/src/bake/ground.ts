// @pxl/shared/bake/ground.ts

/**
 * Which of a cell's four ground channels the viewer actually sees.
 *
 * A baked cell carries land/water/structure/transport at once, and later
 * channels paint over earlier ones. Every consumer that asks "what is this
 * cell" has to agree on that precedence or they answer differently about
 * the same cell: the terrain shader picks a texture layer from it, and the
 * object layer asks whether a placed terrain-styled cube would show
 * anything the terrain does not already show. They disagreed before this
 * was one definition — a road over bare ground reads `land = BARE` while
 * rendering as road, so comparing one channel cancelled a bare-ground cube
 * into a road.
 */

import { GROUND_CH_KEYS, GroundChannelKey } from "./format.ts";
import { cube_types } from "../spec/cell/cube.ts";

export { GROUND_CH_KEYS, GroundChannelKey };

/** Land with nothing painted still renders as bare ground rather than as
 * nothing, so LAND alone has a fallback the other channels do not. */
export const LAND_BASE_TYPE: number = cube_types.land.bare;

export type ShownGround = { channel: GroundChannelKey; type_id: number };

/**
 * `read` returns a channel's raw baked type id (0 = unpainted). STRUCTURE
 * is skipped unless shown, matching the renderer's own dev toggle — a cell
 * whose building is hidden shows what is under it.
 */
export function shown_ground(
  read: (channel: GroundChannelKey) => number,
  show_structures: boolean,
): ShownGround {
  let shown: ShownGround = { channel: GroundChannelKey.LAND, type_id: LAND_BASE_TYPE };
  for (const channel of GROUND_CH_KEYS) {
    if (channel === GroundChannelKey.STRUCTURE && !show_structures) continue;
    const type_id = read(channel);
    if (type_id > 0) shown = { channel, type_id };
  }
  return shown;
}
