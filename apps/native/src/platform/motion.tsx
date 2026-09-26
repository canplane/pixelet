// @pxl/native/platform/motion.tsx

/**
 * A view that moves to the pose it is given, on a device: React Native's
 * native driver, which runs the animation on the UI thread. The script's part
 * is one call per new pose, which hands the whole curve over; no frame of it
 * runs on the JavaScript thread, whose frame the kernel's turn spends.
 *
 * A spring goes over as a timing whose easing is the spring's solved curve
 * (shared/ui/motion/transition.ts), the same curve the page plays, so the two
 * move alike and settle together. A pose that arrives mid-flight starts from
 * the value the UI thread has, so a turn back never jumps.
 */

import { useEffect, useReducer, useRef } from "react";
import { Animated, Easing, type EasingFunction } from "react-native";

import { type MotionProps, type Pose, resolve, type SpinProps } from "@/shared/ui/motion/pose.ts";
import { curveAt, springCurve, type Transition } from "@/shared/ui/motion/transition.ts";

type Channel = keyof Pose;
const CHANNELS: readonly Channel[] = ["opacity", "translateX", "translateY", "scale", "rotate"];

function timing(t: Transition): { duration: number; easing: EasingFunction; delay: number } {
  if (t.type === "ease") return { duration: t.durationMs, easing: Easing.bezier(...t.bezier), delay: t.delayMs ?? 0 };
  const curve = springCurve(t);
  return { duration: curve.durationMs, easing: (p) => curveAt(curve, p), delay: t.delayMs ?? 0 };
}

export function Motion({ to, from, replay = null, transition, onSettled, style, onLayout, children }: MotionProps) {
  const target = resolve(to);
  const initial = resolve(from ?? to);

  const values = useRef<Record<Channel, Animated.Value> | null>(null);
  if (values.current === null) {
    values.current = {
      opacity: new Animated.Value(initial.opacity),
      translateX: new Animated.Value(initial.translateX),
      translateY: new Animated.Value(initial.translateY),
      scale: new Animated.Value(initial.scale),
      rotate: new Animated.Value(initial.rotate),
    };
  }
  const v = values.current;

  const [, commit] = useReducer((n: number) => n + 1, 0);

  /** The pose last handed over; null before the first. */
  const stood = useRef<Required<Pose> | null>(null);
  const replayed = useRef(replay);
  const settled = useRef(onSettled);
  settled.current = onSettled;

  useEffect(() => {
    const replaying = replay !== null && replay !== replayed.current;
    replayed.current = replay;

    let start: Required<Pose> = stood.current ?? initial;
    if (replaying && from) {
      start = resolve(from);
      for (const c of CHANNELS) v[c].setValue(start[c]);
    }
    stood.current = target;

    const { duration, easing, delay } = timing(transition);
    const moving = CHANNELS.filter((c) => start[c] !== target[c]);
    const moves = moving.map((c) => Animated.timing(v[c], { toValue: target[c], duration, easing, delay, useNativeDriver: true }));
    if (moves.length === 0) {
      settled.current?.();
      return;
    }
    // Not stopped together: a later pose that moves one channel takes that
    // channel over and leaves the others running to where they were going.
    Animated.parallel(moves, { stopTogether: false }).start(({ finished }) => {
      if (!finished) return;
      // What the UI thread reached, committed as the view's own props too. A
      // native animation moves the view and not its props, so anything that
      // lays the view out again from its props — a layout animation, as a
      // pill opening runs — put back where the animation had started: a
      // toast faded in stood faint, or blank, once its pill opened.
      for (const c of moving) v[c].setValue(target[c]);
      commit();
      settled.current?.();
    });
    // `transition` and `from` are read when a pose arrives, not watched: a
    // new transition alone moves nothing.
  }, [target.opacity, target.translateX, target.translateY, target.scale, target.rotate, replay]);

  return (
    <Animated.View
      onLayout={onLayout}
      style={[
        style,
        {
          opacity: v.opacity,
          transform: [
            { translateX: v.translateX },
            { translateY: v.translateY },
            { scale: v.scale },
            { rotate: v.rotate.interpolate({ inputRange: [0, 1], outputRange: ["0deg", "1deg"] }) },
          ],
        },
      ]}
    >
      {children}
    </Animated.View>
  );
}

/** A view turning round and round, once a period, at an even rate: a load
 * that has not said how far along it is. A loop the UI thread runs, for as
 * long as it is mounted. */
export function Spin({ periodMs, children }: SpinProps) {
  const turn = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    turn.setValue(0);
    const loop = Animated.loop(Animated.timing(turn, { toValue: 1, duration: periodMs, easing: Easing.linear, useNativeDriver: true }));
    loop.start();
    return () => loop.stop();
  }, [periodMs]);
  return <Animated.View style={{ transform: [{ rotate: turn.interpolate({ inputRange: [0, 1], outputRange: ["0deg", "360deg"] }) }] }}>{children}</Animated.View>;
}
