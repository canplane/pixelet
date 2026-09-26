// @pxl/web/feature/map/ui/busy-dim.tsx

import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, type ViewStyle } from "react-native";

import { HANDHELD } from "@platform/core.ts";
import { Motion } from "@platform/motion.tsx";
import { useKernel } from "@/kernel/shell.ts";
import { Dim } from "@/shared/ui/motion/veil.ts";
import { DIM } from "@/shared/ui/motion/transition.ts";

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
      <Motion to={busy ? { scale: 0.98 } : {}} transition={DIM} style={[style, busy ? styles.block : styles.pass]}>
        {children}
      </Motion>
    </Dim.Provider>
  );
}

const styles = StyleSheet.create({
  // From a style sheet, not inline: react-native-web draws a sheet's
  // `box-none` as a class that gives the children back their touches, and an
  // inline one as a plain `none` that every control under it inherits.
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
});
