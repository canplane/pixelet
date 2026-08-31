// @pxl/shared/assets/decor/boats.ts

// Same bridging role as ./trees.ts — see that file's own doc comment.
// Curated to small/medium craft only (a rowboat, a small fishing boat, a
// sailboat, a speedboat): the watercraft-pack's own cargo ships and ocean
// liners are landmark-scale, not "scattered background decoration" scale.
import boat_row_small_url from "./kenney_watercraft_pack/boat-row-small.glb?url";
import boat_fishing_small_url from "./kenney_watercraft_pack/boat-fishing-small.glb?url";
import boat_sail_a_url from "./kenney_watercraft_pack/boat-sail-a.glb?url";
import boat_speed_a_url from "./kenney_watercraft_pack/boat-speed-a.glb?url";
import boat_skin_url from "./kenney_watercraft_pack/colormap.png";

export const BOAT_SKIN_URL: string = boat_skin_url;

export const BOAT_MODEL_URLS: readonly string[] = [
  boat_row_small_url,
  boat_fishing_small_url,
  boat_sail_a_url,
  boat_speed_a_url,
];
