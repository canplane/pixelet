// @pxl/client/ui/world/busy-dim.tsx

import type { ReactNode } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@/ui/component/motion/motion.tsx";
import { Dim } from "@/ui/component/motion/dim.ts";
import { DIM } from "@/ui/component/motion/transition.ts";
import { touches } from "@/ui/component/touch.ts";
import * as device from "@/service/device.ts";
import { useCore } from "@/ui/store/watch.ts";

/**
 * On a desktop, the chrome dims and takes no clicks while the world is dragged:
 * releasing a drag over a control clicks it. A handheld's finger cannot do
 * that, so it is left alone.
 */
export function BusyDim({
  style,
  children,
}: {
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}) {
  const busy = useCore((s) => s.camera_busy) && !device.handheld;
  return (
    <Dim.Provider value={busy ? 0.4 : 1}>
      <Motion
        to={busy ? { scale: 0.98 } : {}}
        transition={DIM}
        style={[style, busy ? touches.block : touches.pass]}
      >
        {children}
      </Motion>
    </Dim.Provider>
  );
}
