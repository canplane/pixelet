// @pxl/web/modules/world/render/config.ts

import { VIEW_DIST } from "@/modules/camera/config.ts";

/** Render-distance and fog configuration. */
export const render_cfg = {
  FADE_START: VIEW_DIST * 0.75,
  FADE_END: VIEW_DIST,

  FAR_CLIP: VIEW_DIST * 1.5,

  FOG_NEAR: VIEW_DIST * 0.75,
  FOG_FAR: VIEW_DIST,
} as const;
