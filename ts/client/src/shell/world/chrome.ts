// @pxl/client/shell/world/chrome.ts

import { chrome } from "@/shell/ui/theme.ts";
import { bracket } from "@/bracket.tsx";

/** Where the map's top row of controls stands. */
export function useChromeTop(): number {
  return chrome.inset + bracket.view.useSafeArea().top +
    bracket.view.useViewportTop();
}
