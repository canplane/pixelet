// @pxl/web/shared/ui/motion/veil.ts

/**
 * A surface's presence (1 to 0), told to the surface rather than applied as a
 * parent opacity: UIKit renders glass wrongly under any alpha below one, so a
 * glass toast stood solid and then vanished.
 */

import { createContext, useContext, useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/abi/word.ts";
import { DIM, type Transition } from "./transition.ts";

export const Dim = createContext<f64>(1);

/** A change of dim moves as the dim does; anything else as the surface's own fade. */
export function useVeil(fade: f64, transition: Transition): { level: f64; transition: Transition } {
  const dim = useContext(Dim);
  const last = useRef(dim);
  const dimming = dim !== last.current;
  useEffect(() => {
    last.current = dim;
  });
  return { level: fade * dim, transition: dimming ? DIM : transition };
}
