// @pxl/web/feature/world/ui/busy-dim.tsx

import type { ReactNode } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { HANDHELD } from "@platform/core.ts";
import { Motion } from "@platform/motion.tsx";
import { useKernel } from "@/kernel/shell.ts";
import { Dim } from "@/shared/ui/motion/veil.ts";
import { DIM } from "@/shared/ui/motion/transition.ts";
import { touches } from "@/shared/ui/touch.ts";

/**
 * On a desktop, the chrome dims and takes no clicks while the map is dragged:
 * releasing a drag over a control clicks it. A handheld's finger cannot do
 * that, so it is left alone.
 */
export function BusyDim({ style, children }: { style?: StyleProp<ViewStyle>; children: ReactNode }) {
  const busy = useKernel((s) => s.camera_busy) && !HANDHELD;
  return (
    <Dim.Provider value={busy ? 0.4 : 1}>
      <Motion to={busy ? { scale: 0.98 } : {}} transition={DIM} style={[style, busy ? touches.block : touches.pass]}>
        {children}
      </Motion>
    </Dim.Provider>
  );
}
