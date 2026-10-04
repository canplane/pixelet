// @pxl/web/platform/web/backing.tsx

/**
 * A floating control's backing on the web (and Android): its fill. A page
 * has no system glass, so presence is plain opacity.
 */

import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import type { BackingProps } from "@/shared/ui/backing.ts";
import { backdrop } from "@platform/backdrop.ts";

export const SYSTEM_BACKING: boolean = false;

export function Backing({ fill, material, fade = 1, fadeFrom, transition = SNAP, style, children }: BackingProps) {
  const veil = useVeil(fade, transition, fadeFrom);
  return (
    <Motion
      {...veil.motion}
      style={[style, material ? backdrop(fill as string, material) : { backgroundColor: fill }]}
    >
      {children}
    </Motion>
  );
}
