// @pxl/web/feature/map/ui/overlay/overlays.tsx

import { useVisualViewport } from "@platform/viewport.ts";

import { TopOverlays } from "./top-overlays.tsx";
import { BottomOverlays } from "./bottom-overlays.tsx";

export function Overlays() {
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <BottomOverlays />
    </>
  );
}
