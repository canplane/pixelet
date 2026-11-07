// @pxl/client/ui/world/chrome.ts

import { chrome } from "@/ui/component/theme.ts";
import { useSafeArea } from "@/service/safe-area.ts";
import { useViewportTop } from "@/service/viewport.ts";

/** Where the world's top row of controls stands. */
export function useChromeTop(): number {
  return chrome.inset + useSafeArea().top + useViewportTop();
}
