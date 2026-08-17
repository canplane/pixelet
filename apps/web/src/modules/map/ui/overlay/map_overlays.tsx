// @pxl/web/modules/map/ui/overlay/map_overlays.tsx

import { useVisualViewport } from "./use_visual_viewport.ts";

import { TopOverlays } from "./top_overlays.tsx";
import { BottomOverlays } from "./bottom_overlays.tsx";

/**
 * Manages every 2D UI component rendered above the 3D canvas.
 * See top_overlays.tsx/bottom_overlays.tsx's own doc comments for why
 * they're split into top/bottom — worth splitting once the bottom side
 * grew its own layout rules (a shared safe area) centered on the mode bar.
 */
export function MapOverlays() {
  // Publishes --vv-top / --vv-bottom for the two overlay roots below; see the
  // hook for why iOS needs it even with the viewport meta in place.
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <BottomOverlays />
    </>
  );
}
