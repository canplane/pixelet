// @pxl/web/feature/map/ui/busy-dim.tsx

import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, type ViewStyle } from "react-native";

import { HANDHELD } from "@platform/core.ts";
import { Motion } from "@platform/motion.tsx";
import { useKernel } from "@/kernel/shell.ts";
import { Dim } from "@/shared/ui/motion/veil.ts";
import { DIM } from "@/shared/ui/motion/transition.ts";

/**
 * On a desktop, the chrome dims and takes no clicks while the map is dragged:
 * releasing a drag over a control clicks it. A handheld's finger cannot do
 * that, so it is left alone.
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
  // From a sheet: RN-web draws an inline `box-none` as `none`, which every control under it inherits.
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
});
