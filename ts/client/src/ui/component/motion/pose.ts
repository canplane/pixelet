// @pxl/client/ui/component/motion/pose.ts

/**
 * A pose is where a moving view stands: its opacity and its transform, and
 * nothing else. A change of pose therefore never reflows the layout, and iOS
 * WebKit leaves no ghost of the move.
 */

import { type ReactNode, useRef } from "react";
import type { LayoutChangeEvent, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Transition } from "./transition.ts";

/** A channel left out takes its rest value: fully opaque, not translated, at
 * scale 1 and not rotated. `rotate` is in degrees. */
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
  /** A change to a new non-null value plays the motion again from `from`. */
  replay?: Replay;
  transition: Transition;
  onSettled?: () => void;
  style?: StyleProp<ViewStyle>;
  onLayout?: (e: LayoutChangeEvent) => void;
  children?: ReactNode;
}

/** Returns a check of whether the current run of a Motion's effect is a
 * replay. Call it once in each run: it records the value it has seen. */
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
