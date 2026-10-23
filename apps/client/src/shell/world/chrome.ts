// @pxl/client/shell/world/chrome.ts

import { useSafeArea } from "@/bracket.tsx";
import { useViewportTop } from "@/bracket.tsx";
import { chrome } from "@/shell/ui/theme.ts";

/** Where the map's top row of controls stands. */
export function useChromeTop(): number {
  return chrome.inset + useSafeArea().top + useViewportTop();
}
