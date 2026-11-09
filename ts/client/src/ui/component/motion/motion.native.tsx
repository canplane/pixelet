// @pxl/client/ui/component/motion/motion.native.tsx

/**
 * A view on a device, moved by React Native's native driver. One call hands
 * the whole curve to the UI thread, because the core spends the JS thread's
 * frame time. A pose given while an animation runs starts from the value the
 * UI thread holds.
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

  const rest = useRef<Required<Pose> | null>(null);
  const replays = useReplay(replay);
  const settled = useRef(onSettled);
  settled.current = onSettled;

  useEffect(() => {
    const replaying = replays();

    let start: Required<Pose> = rest.current ?? init;
    if ((replaying || rest.current === null) && from) {
      start = resolve(from);
      for (const c of CHANNELS) v[c].setValue(start[c]);
    }
    rest.current = target;

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
    // A later pose that moves one channel leaves the other channels running.
    Animated.parallel(moves, { stopTogether: false }).start(({ finished }) => {
      if (!finished) return;
      // The end values are committed as props as well. A layout animation lays
      // the view out again from its props, and without them a faded-in toast
      // turns faint or blank once its pill opens.
      for (const c of moving) v[c].setValue(target[c]);
      commit();
      settled.current?.();
    });
  }, [...CHANNELS.map((c) => target[c]), replay]);

  // Under StrictMode, React takes a view down and puts it back. The view put
  // back must arrive from `from` again.
  useEffect(
    () => () => {
      for (const c of CHANNELS) v[c].stopAnimation();
      rest.current = null;
    },
    [],
  );

  // `needsOffscreenAlphaCompositing` fades the view as one layer. Without it,
  // Android fades each child on its own, and overlapping children show
  // through one another.
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
