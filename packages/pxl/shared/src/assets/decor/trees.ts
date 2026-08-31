// @pxl/shared/assets/decor/trees.ts

// Bridges real model files to typed URLs — same reasoning
// ../characters/variants.ts's own doc comment gives for its imports: this
// file's whole job is that bridge, so it sits with the assets it bridges,
// not with the scatter/placement logic that consumes it (apps/web's
// modules/world/decor/). One import per .glb, not a glob — see
// variants.ts's own doc comment for why import.meta.glob silently returns
// no matches once the pattern crosses out of this file's own directory
// tree.
import tree_url from "./kenney_fantasy_town/tree.glb?url";
import tree_high_url from "./kenney_fantasy_town/tree-high.glb?url";
import tree_crooked_url from "./kenney_fantasy_town/tree-crooked.glb?url";
import tree_high_round_url from "./kenney_fantasy_town/tree-high-round.glb?url";
import tree_high_crooked_url from "./kenney_fantasy_town/tree-high-crooked.glb?url";
import tree_skin_url from "./kenney_fantasy_town/colormap.png";

/** All five Kenney "Fantasy Town Kit" tree variants share this one colormap
 * (same shared-atlas convention the pack uses for every other prop in it —
 * fences, chimneys, banners, ... — the reason this file lives right next to
 * where those would eventually go too). */
export const TREE_SKIN_URL: string = tree_skin_url;

/** Order is arbitrary — decor/scatter.ts picks among these by index via its
 * own deterministic per-cell RNG, not by any meaning attached to position. */
export const TREE_MODEL_URLS: readonly string[] = [
  tree_url,
  tree_high_url,
  tree_crooked_url,
  tree_high_round_url,
  tree_high_crooked_url,
];
