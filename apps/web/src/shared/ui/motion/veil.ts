// @pxl/web/shared/ui/motion/veil.ts

/**
 * How present a surface is, from 1 (all there) to 0 (gone), said to the
 * surface rather than done to it.
 *
 * A fade is not an opacity laid over a subtree, because a device's glass
 * cannot be seen through a parent's opacity: UIKit renders a visual effect
 * wrongly under any alpha below one, or skips it and never puts it back, so
 * a glass toast fading out stood solid and then vanished. So whatever fades —
 * a toast's card, the chrome dimmed while the camera is busy — says how
 * present it is, and each surface under it shows that its own way
 * (@platform glass.tsx and pill.tsx): a page by opacity, a device's glass by
 * its own style, animated by the system.
 */

import { createContext, useContext, useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/abi/word.ts";
import { DIM, type Transition } from "./transition.ts";

/** How dim the chrome around a surface is: 1 but while the camera is busy
 * (feature/map/ui/busy-dim.tsx). */
export const Dim = createContext<f64>(1);

/** A surface's presence, its own fade and the chrome's dim together, and how
 * to move to it: a change of dim moves as the dim does, anything else as the
 * surface's own fade does. */
export function useVeil(fade: f64, transition: Transition): { level: f64; transition: Transition } {
  const dim = useContext(Dim);
  const last = useRef(dim);
  const dimming = dim !== last.current;
  useEffect(() => {
    last.current = dim;
  });
  return { level: fade * dim, transition: dimming ? DIM : transition };
}
