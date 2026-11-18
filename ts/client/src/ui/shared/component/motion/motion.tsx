// @pxl/client/ui/shared/component/motion/motion.tsx

/**
 * A view moved by react-native-reanimated's CSS transitions, which run off the
 * JS thread on the web and natively alike. The view's style is always the pose
 * it was given last, and a pose given while a transition runs starts from
 * where the view is.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import Animated, { cubicBezier } from "react-native-reanimated";
import { StyleSheet, type ViewStyle } from "react-native";

import {
  CHANNELS,
  type MotionProps,
  type Pose,
  resolve,
  type SpinProps,
  useReplay,
} from "@/ui/shared/component/motion/pose.ts";

function styleOf(p: Required<Pose>): ViewStyle {
  return {
    opacity: p.opacity,
    transform: [
      { translateX: p.translateX },
      { translateY: p.translateY },
      { scale: p.scale },
      { rotate: `${p.rotate}deg` },
    ],
  };
}

function same(a: Required<Pose>, b: Required<Pose>): boolean {
  return CHANNELS.every((c) => a[c] === b[c]);
}

/** A pose taken at once: a transition of no property, not none at all, so
 * that Reanimated keeps the pose as where the next transition starts from.
 * Its duration is given because Reanimated's check of a transition asks for
 * one beside its property. */
const STILL = { transitionProperty: "none", transitionDuration: 0 } as const;

export function Motion({
  to,
  from,
  replay = null,
  transition,
  onSettled,
  style,
  onLayout,
  children,
}: MotionProps) {
  const target = resolve(to);
  // The view stands at `from` for a frame, without a transition, and moves to
  // its pose from there: as it mounts, and at each replay.
  const [starting, setStarting] = useState(from !== undefined);
  const replays = useReplay(replay);
  useLayoutEffect(() => {
    if (replays() && from) setStarting(true);
  }, [replay]);
  useEffect(() => {
    if (!starting) return;
    const frame = requestAnimationFrame(() => setStarting(false));
    return () => cancelAnimationFrame(frame);
  }, [starting]);

  const at = starting && from ? resolve(from) : target;
  const settled = useRef(onSettled);
  settled.current = onSettled;
  // The pose the view last stood at. A move settles when its transition's
  // time is up, and a pose that moves nothing settles at once.
  const last = useRef<Required<Pose> | null>(null);
  useEffect(() => {
    const previous = last.current;
    last.current = at;
    if (starting) return;
    const moves = previous !== null && !same(previous, at);
    const timer = setTimeout(
      () => settled.current?.(),
      moves ? transition.durationMs + (transition.delayMs ?? 0) : 0,
    );
    return () => clearTimeout(timer);
  }, [starting, ...CHANNELS.map((c) => target[c])]);

  // The animated view writes its style inline, where react-native-web draws
  // a `box-none` as `none`, so what touches the view takes goes as the prop,
  // which draws it rightly.
  const { pointerEvents, ...rest } = StyleSheet.flatten(style) ?? {};

  // `needsOffscreenAlphaCompositing` fades the view as one layer. Without it,
  // Android fades each child on its own, and overlapping children show
  // through one another.
  return (
    <Animated.View
      onLayout={onLayout}
      pointerEvents={pointerEvents}
      needsOffscreenAlphaCompositing
      style={[
        rest,
        styleOf(at),
        starting
          ? STILL
          : {
              transitionProperty: ["opacity", "transform"],
              transitionDuration: transition.durationMs,
              transitionDelay: transition.delayMs ?? 0,
              transitionTimingFunction: cubicBezier(...transition.bezier),
            },
      ]}
    >
      {children}
    </Animated.View>
  );
}

const TURN = {
  from: { transform: [{ rotate: "0deg" }] },
  to: { transform: [{ rotate: "360deg" }] },
};

export function Spin({ periodMs, children }: SpinProps) {
  return (
    <Animated.View
      style={{
        animationName: TURN,
        animationDuration: periodMs,
        animationIterationCount: "infinite",
        animationTimingFunction: "linear",
      }}
    >
      {children}
    </Animated.View>
  );
}
