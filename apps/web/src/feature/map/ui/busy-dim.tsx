// @pxl/web/feature/map/ui/busy-dim.tsx

import type { ReactNode } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { HANDHELD } from "@platform/core.ts";
import { Motion } from "@platform/motion.tsx";
import { useKernel } from "@/kernel/shell.ts";
import { Dim } from "@/shared/ui/motion/veil.ts";
import { SLIDE } from "@/shared/ui/motion/transition.ts";

/**
 * Map chrome while the map is dragged (`camera_busy`), on a desktop: dimmed
 * and taking no clicks, since a mouse that lets go of a drag over a control
 * clicks it. On a handheld the finger dragging the map is the map's and
 * reaches no control, and a second finger on one means to press it, so the
 * chrome stays as it is, as a phone's maps keep theirs. The one treatment
 * for the top overlays and the bottom alike. The dim is said to what stands
 * under it (shared/ui/motion/veil.ts) rather than laid over it, which a
 * device's glass could not be seen through.
 */
export function BusyDim({ style, children }: { style?: StyleProp<ViewStyle>; children: ReactNode }) {
  const busy = useKernel((s) => s.camera_busy) && !HANDHELD;
  return (
    <Dim.Provider value={busy ? 0.4 : 1}>
      <Motion to={busy ? { scale: 0.98 } : {}} transition={SLIDE} style={[style, { pointerEvents: busy ? "none" : "box-none" }]}>
        {children}
      </Motion>
    </Dim.Provider>
  );
}
