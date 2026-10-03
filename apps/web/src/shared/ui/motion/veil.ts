// @pxl/web/shared/ui/motion/veil.ts

/**
 * A surface's presence (1 to 0), told to the surface rather than applied as a
 * parent opacity: UIKit renders glass wrongly under any alpha below one, so a
 * glass toast stood solid and then vanished.
 */

import { createContext, useContext, useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/abi/word.ts";
import type { Pose } from "./pose.ts";
import { DIM, type Transition } from "./transition.ts";

export const Dim = createContext<f64>(1);

interface Veil {
  level: f64;
  /** A Motion's props for it. */
  motion: { from?: Pose; to: Pose; transition: Transition };
}

/** A change of dim moves as the dim does; anything else as the surface's own fade. */
export function useVeil(fade: f64, transition: Transition, fadeFrom?: f64): Veil {
  const dim = useContext(Dim);
  const last = useRef(dim);
  const dimming = dim !== last.current;
  useEffect(() => {
    last.current = dim;
  });
  const level = fade * dim;
  return {
    level,
    motion: { from: fadeFrom === undefined ? undefined : { opacity: fadeFrom }, to: { opacity: level }, transition: dimming ? DIM : transition },
  };
}
