// @pxl/client/shared/ui/corner-card.ts

import { alpha, color } from "@/shared/ui/theme.ts";

/** A card of the window under the settings button (`feature/world/ui/corner.tsx`): the chat's
 * lines, bare against the button's edge as the other words over the world are, or a debug panel. */
export const cornerCard = {
  width: 280,
  marginBottom: 8,
} as const;

/** A panel's card, its words inset on a backing the chat's do without. */
export const cornerPanel = {
  ...cornerCard,
  padding: 10,
  borderRadius: 8,
  backgroundColor: alpha(color.black, 0.72),
} as const;
