// @pxl/shared/assets/decor/graveyard_trees.ts

// Same bridging role as ./trees.ts, for the graveyard-kit's own pine
// variants — a separate file (not added to trees.ts) because this pack has
// its own colormap atlas, not the fantasy-town-kit's; tree_decor.ts is what
// actually mixes the two packs into one kind's variant list.
import pine_url from "./kenney_graveyard_kit/pine.glb?url";
import pine_crooked_url from "./kenney_graveyard_kit/pine-crooked.glb?url";
import pine_fall_url from "./kenney_graveyard_kit/pine-fall.glb?url";
import pine_fall_crooked_url from "./kenney_graveyard_kit/pine-fall-crooked.glb?url";
import pine_skin_url from "./kenney_graveyard_kit/colormap.png";

export const PINE_SKIN_URL: string = pine_skin_url;

/** Two plain green pines plus two "fall" (ochre/autumn) ones — the ochre
 * variants are what actually motivated pulling this pack in alongside
 * assets/decor/trees.ts's fantasy-town-kit set. */
export const PINE_MODEL_URLS: readonly string[] = [
  pine_url,
  pine_crooked_url,
  pine_fall_url,
  pine_fall_crooked_url,
];
