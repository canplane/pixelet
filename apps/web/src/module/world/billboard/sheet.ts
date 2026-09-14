// @pxl/web/module/world/billboard/sheet.ts

import { BillboardAtlas } from "./atlas.ts";

/**
 * Draw order among billboard sets (crates/pxl-core/src/billboard.rs), for
 * the case two billboards land on the same pixels at the same depth. Depth
 * testing settles everything else; this only settles ties, and the tie is
 * between a badge that labels a fixed place and a bubble somebody is saying
 * right now. The bubble wins — it is transient and the badge will still be
 * there afterwards.
 */
export const BILLBOARD_ORDER = {
  poi: 0,
  chat: 1,
} as const;

/**
 * The one sheet every billboard draws into — POI badges and the chat
 * bubble alike — and the one the WebGPU renderer mirrors
 * (render/gpu/badges.ts). One sheet is what lets every billboard set share
 * a pipeline and a binding: a second sheet would be a second pass's worth
 * of state for a bubble or two.
 */
export const billboard_sheet = new BillboardAtlas();
