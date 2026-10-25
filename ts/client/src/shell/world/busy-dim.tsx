// @pxl/client/shell/world/busy-dim.tsx

import type { ReactNode } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import { Dim } from "@/shell/ui/motion/veil.ts";
import { DIM } from "@/shell/ui/motion/transition.ts";
import { touches } from "@/shell/ui/touch.ts";
import { bracket } from "@/bracket.tsx";
import { useKernel } from "@/shell/store/watch.ts";

/**
 * On a desktop, the chrome dims and takes no clicks while the map is dragged:
 * releasing a drag over a control clicks it. A handheld's finger cannot do
 * that, so it is left alone.
 */
export function BusyDim({ style, children }: { style?: StyleProp<ViewStyle>; children: ReactNode }) {
  const busy = useKernel((s) => s.camera_busy) && !bracket.device.handheld;
  return (
    <Dim.Provider value={busy ? 0.4 : 1}>
      <Motion to={busy ? { scale: 0.98 } : {}} transition={DIM} style={[style, busy ? touches.block : touches.pass]}>
        {children}
      </Motion>
    </Dim.Provider>
  );
}
