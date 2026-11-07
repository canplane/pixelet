// @pxl/client/ui/component/motion/motion.native.tsx

/**
 * A view moved by RN's native driver: one call hands the whole curve to the
 * UI thread, since the core spends the JS thread's frame. A pose arriving
 * mid-flight starts from the UI thread's value.
 */

import { useEffect, useReducer, useRef } from "react";
import { Animated, Easing, type EasingFunction } from "react-native";

import {
  CHANNELS,
  type MotionProps,
  type Pose,
  resolve,
  type SpinProps,
  useReplay,
} from "@/ui/component/motion/pose.ts";
import { curveAt, type Transition } from "@/ui/component/motion/transition.ts";

function timing(t: Transition): {
  duration: number;
  easing: EasingFunction;
  delay: number;
} {
  return {
    duration: t.durationMs,
    easing: (p) => curveAt(t, p),
    delay: t.delayMs ?? 0,
  };
}

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
  const init = resolve(from ?? to);

  const values = useRef<Record<keyof Pose, Animated.Value> | null>(null);
  if (values.current === null) {
    values.current = Object.fromEntries(
      CHANNELS.map((c) => [c, new Animated.Value(init[c])]),
    ) as Record<keyof Pose, Animated.Value>;
  }
  const v = values.current;

  const [, commit] = useReducer((n: number) => n + 1, 0);

  const stood = useRef<Required<Pose> | null>(null);
  const replays = useReplay(replay);
  const settled = useRef(onSettled);
  settled.current = onSettled;

  useEffect(() => {
    const replaying = replays();

    let start: Required<Pose> = stood.current ?? init;
    if ((replaying || stood.current === null) && from) {
      start = resolve(from);
      for (const c of CHANNELS) v[c].setValue(start[c]);
    }
    stood.current = target;

    const { duration, easing, delay } = timing(transition);
    const moving = CHANNELS.filter((c) => start[c] !== target[c]);
    const moves = moving.map((c) =>
      Animated.timing(v[c], {
        toValue: target[c],
        duration,
        easing,
        delay,
        useNativeDriver: true,
      }),
    );
    if (moves.length === 0) {
      settled.current?.();
      return;
    }
    // A later pose moving one channel leaves the others running.
    Animated.parallel(moves, { stopTogether: false }).start(({ finished }) => {
      if (!finished) return;
      // Committed as props too: a layout animation re-lays the view from its
      // props, which left a faded-in toast faint or blank once its pill opened.
      for (const c of moving) v[c].setValue(target[c]);
      commit();
      settled.current?.();
    });
  }, [...CHANNELS.map((c) => target[c]), replay]);

  // Under StrictMode a view put back arrives again from `from`.
  useEffect(
    () => () => {
      for (const c of CHANNELS) v[c].stopAnimation();
      stood.current = null;
    },
    [],
  );

  // One layer while not opaque: Android otherwise fades each child apart, and
  // overlapping ones show through.
  return (
    <Animated.View
      onLayout={onLayout}
      needsOffscreenAlphaCompositing
      style={[
        style,
        {
          opacity: v.opacity,
          transform: [
            { translateX: v.translateX },
            { translateY: v.translateY },
            { scale: v.scale },
            {
              rotate: v.rotate.interpolate({
                inputRange: [0, 1],
                outputRange: ["0deg", "1deg"],
              }),
            },
          ],
        },
      ]}
    >
      {children}
    </Animated.View>
  );
}

export function Spin({ periodMs, children }: SpinProps) {
  const turn = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    turn.setValue(0);
    const loop = Animated.loop(
      Animated.timing(turn, {
        toValue: 1,
        duration: periodMs,
        easing: Easing.linear,
        useNativeDriver: true,
      }),
    );
    loop.start();
    return () => loop.stop();
  }, [periodMs]);
  return (
    <Animated.View
      style={{
        transform: [
          {
            rotate: turn.interpolate({
              inputRange: [0, 1],
              outputRange: ["0deg", "360deg"],
            }),
          },
        ],
      }}
    >
      {children}
    </Animated.View>
  );
}
