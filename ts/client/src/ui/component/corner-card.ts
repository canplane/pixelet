// @pxl/client/ui/component/corner-card.ts

import { alpha, color } from "@/ui/component/theme.ts";

/** A card in the corner under the settings button holds the chat's lines or a
 * debug panel. The chat's lines stand bare against the button's edge, as the
 * other words over the world do. */
export const cornerCard = {
  width: 280,
  marginBottom: 8,
} as const;

export const cornerPanel = {
  ...cornerCard,
  padding: 10,
  borderRadius: 8,
  backgroundColor: alpha(color.black, 0.72),
} as const;
