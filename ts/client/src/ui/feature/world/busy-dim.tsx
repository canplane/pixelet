// @pxl/client/ui/feature/world/busy-dim.tsx

import type { ReactNode } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { Dim } from "@/ui/shared/component/motion/dim.ts";
import { DIM } from "@/ui/shared/component/motion/transition.ts";
import { TOUCH } from "@/ui/shared/component/touch.ts";
import * as device from "@/service/platform/device.ts";
import { useCore } from "@/ui/shared/watch.ts";

/**
 * On a desktop, the chrome dims and takes no clicks while the world is
 * dragged, because releasing a drag over a control would click it. A
 * handheld's finger cannot do that, so on a handheld the chrome is left alone.
 */
export function BusyDim({
  style,
  children,
}: {
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}) {
  const busy = useCore((s) => s.camera_busy) && !device.HANDHELD;
  return (
    <Dim.Provider value={busy ? 0.4 : 1}>
      <Motion
        to={busy ? { scale: 0.98 } : {}}
        transition={DIM}
        style={[style, busy ? TOUCH.block : TOUCH.pass]}
      >
        {children}
      </Motion>
    </Dim.Provider>
  );
}
