// @pxl/client/shell/world/chrome.ts

import { chrome } from "@/shell/ui/theme.ts";
import { useSafeArea } from "@/services/safe-area.ts";
import { useViewportTop } from "@/services/viewport.ts";

/** Where the world's top row of controls stands. */
export function useChromeTop(): number {
  return chrome.inset + useSafeArea().top + useViewportTop();
}
