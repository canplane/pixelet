// @pxl/web/module/world/billboard/sheet.ts

import { BillboardAtlas } from "./atlas.ts";

/**
 * The one sheet every billboard draws into — POI badges and the chat
 * bubble alike — and the one the WebGPU renderer mirrors
 * (render/gpu/badges.ts). One sheet is what lets every billboard set share
 * a pipeline and a binding: a second sheet would be a second pass's worth
 * of state for a bubble or two.
 */
export const billboard_sheet = new BillboardAtlas();
