// @pxl/web/feature/map/ui/overlay/all.tsx

import { useVisualViewport } from "@platform/viewport.ts";

import { TopOverlays } from "./top.tsx";
import { BottomOverlays } from "./bottom.tsx";

export function Overlays() {
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <BottomOverlays />
    </>
  );
}
