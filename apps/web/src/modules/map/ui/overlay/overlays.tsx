// @pxl/web/modules/map/ui/overlay/overlays.tsx

import { useVisualViewport } from "./use_visual_viewport.ts";

import { TopOverlays } from "./top_overlays.tsx";
import { BottomOverlays } from "./bottom_overlays.tsx";

export function Overlays() {
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <BottomOverlays />
    </>
  );
}
