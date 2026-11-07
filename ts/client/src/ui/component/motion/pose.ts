// @pxl/client/ui/component/motion/pose.ts

/**
 * Where a moving view stands: opacity and transform only, so a pose never
 * reflows and iOS WebKit has nothing to ghost.
 */

import { type ReactNode, useRef } from "react";
import type { LayoutChangeEvent, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Transition } from "./transition.ts";

/** A channel left out stands at rest: opaque, in place, at scale, upright. */
export interface Pose {
  opacity?: f64;
  translateX?: f64;
  translateY?: f64;
  scale?: f64;
  rotate?: f64;
}

const REST: Required<Pose> = {
  opacity: 1,
  translateX: 0,
  translateY: 0,
  scale: 1,
  rotate: 0,
};

/** Every channel, each moved on its own. */
export const CHANNELS: readonly (keyof Pose)[] = [
  "opacity",
  "translateX",
  "translateY",
  "scale",
  "rotate",
];

export function resolve(p: Pose): Required<Pose> {
  return { ...REST, ...p };
}

type Replay = string | number | null;

export interface MotionProps {
  to: Pose;
  from?: Pose;
  /** A new non-null value starts it over from `from`. */
  replay?: Replay;
  transition: Transition;
  onSettled?: () => void;
  style?: StyleProp<ViewStyle>;
  onLayout?: (e: LayoutChangeEvent) => void;
  children?: ReactNode;
}

/** Whether this run of a Motion's effect is a replay, asked once a run. */
export function useReplay(replay: Replay): () => boolean {
  const replayed = useRef(replay);
  return () => {
    const replaying = replay !== null && replay !== replayed.current;
    replayed.current = replay;
    return replaying;
  };
}

export interface SpinProps {
  periodMs: f64;
  children: ReactNode;
}
