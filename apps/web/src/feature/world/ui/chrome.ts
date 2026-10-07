// @pxl/web/feature/world/ui/chrome.ts

import { useSafeArea } from "@platform/safe-area.ts";
import { useViewportTop } from "@platform/viewport.ts";
import { chrome } from "@/shared/ui/theme.ts";

/** Where the map's top row of controls stands. */
export function useChromeTop(): number {
  return chrome.inset + useSafeArea().top + useViewportTop();
}
