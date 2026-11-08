// @pxl/client/ui/world/chrome.ts

import { chrome } from "@/ui/component/theme.ts";
import { useSafeArea } from "@/service/platform/safe-area.ts";
import { useViewportTop } from "@/service/platform/viewport.ts";

/** The top of the world's top row of controls, in pixels from the top of the
 * window. */
export function useChromeTop(): number {
  return chrome.inset + useSafeArea().top + useViewportTop();
}
