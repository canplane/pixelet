// @pxl/web/shared/ui/motion/pose.ts

/**
 * What a moving view is told: where to stand, not how to get there. Only
 * what a compositor moves without laying anything out — opacity and a
 * transform — so a pose never reflows the page or a device's view tree, and
 * iOS's WebKit, which ghosted a width tween over the canvas, has nothing to
 * ghost.
 */

import type { ReactNode } from "react";
import type { LayoutChangeEvent, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import type { Transition } from "./transition.ts";

/** A channel left out stands at rest: opaque, in place, at scale, upright. */
export interface Pose {
  opacity?: f64;
  translateX?: f64;
  translateY?: f64;
  scale?: f64;
  /** Degrees. */
  rotate?: f64;
}

export const REST: Required<Pose> = { opacity: 1, translateX: 0, translateY: 0, scale: 1, rotate: 0 };

export function resolve(p: Pose): Required<Pose> {
  return { ...REST, ...p };
}

export interface MotionProps {
  /** Where it stands; a new pose is moved to from wherever it stands now. */
  to: Pose;
  /** Where it starts when it mounts, and again on each `replay`. */
  from?: Pose;
  /** A new value other than null starts it over from `from`. */
  replay?: string | number | null;
  transition: Transition;
  /** The pose last given has been reached, or was already where it stood. */
  onSettled?: () => void;
  style?: StyleProp<ViewStyle>;
  onLayout?: (e: LayoutChangeEvent) => void;
  children?: ReactNode;
}

export interface SpinProps {
  /** One turn, in milliseconds. */
  periodMs: f64;
  children: ReactNode;
}
