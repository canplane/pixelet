// @pxl/client/ui/shared/component/corner-card.ts

import { alpha, COLOR } from "@/ui/shared/component/theme.ts";

/** A card in the corner under the settings button holds the chat's lines or a
 * debug panel. The chat's lines stand bare against the button's edge, as the
 * other words over the world do. */
export const CORNER_CARD = {
  width: 280,
  marginBottom: 8,
} as const;

export const CORNER_PANEL = {
  ...CORNER_CARD,
  padding: 10,
  borderRadius: 8,
  backgroundColor: alpha(COLOR.black, 0.72),
} as const;
