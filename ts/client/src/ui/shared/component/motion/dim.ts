// @pxl/client/ui/shared/component/motion/dim.ts

/**
 * How present a surface is, from 1 (fully shown) to 0 (gone). A parent tells
 * its surfaces the dim rather than applying it as its own opacity, because
 * UIKit renders glass wrongly under any alpha below one.
 */

import { createContext, useContext, useEffect, useRef } from "react";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Pose } from "./pose.ts";
import { DIM, type Transition } from "./transition.ts";

export const Dim = createContext<f64>(1);

interface Dimmed {
  level: f64;
  motion: { from?: Pose; to: Pose; transition: Transition };
}

export function useDim(
  fade: f64,
  transition: Transition,
  fadeFrom?: f64,
): Dimmed {
  const dim = useContext(Dim);
  const last = useRef(dim);
  const dimming = dim !== last.current;
  useEffect(() => {
    last.current = dim;
  });
  const level = fade * dim;
  return {
    level,
    motion: {
      from: fadeFrom === undefined ? undefined : { opacity: fadeFrom },
      to: { opacity: level },
      transition: dimming ? DIM : transition,
    },
  };
}
