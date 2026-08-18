// @pxl/web/modules/map/ui/overlay/map_overlays.tsx

import { useVisualViewport } from "./use_visual_viewport.ts";

import { TopOverlays } from "./top_overlays.tsx";
import { BottomOverlays } from "./bottom_overlays.tsx";

export function MapOverlays() {
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <BottomOverlays />
    </>
  );
}
