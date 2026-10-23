// @pxl/client/shell/ui/motion/motion.tsx

/**
 * A view moved by Web Animations on the compositor. The committed style is
 * always the given pose, so a cancel leaves the view where it was told to be;
 * a mid-flight pose starts from the computed style.
 */

import { useLayoutEffect, useRef } from "react";
import { View } from "react-native";

import { type MotionProps, type Pose, resolve, type SpinProps } from "@/shell/ui/motion/pose.ts";
import type { Transition } from "@/shell/ui/motion/transition.ts";

function keyframe(p: Required<Pose>): Keyframe {
  return {
    opacity: String(p.opacity),
    transform: `translateX(${p.translateX}px) translateY(${p.translateY}px) scale(${p.scale}) rotate(${p.rotate}deg)`,
  };
}

function same(a: Required<Pose>, b: Required<Pose>): boolean {
  return a.opacity === b.opacity && a.translateX === b.translateX && a.translateY === b.translateY && a.scale === b.scale && a.rotate === b.rotate;
}

function timing(t: Transition): KeyframeAnimationOptions {
  return { delay: t.delayMs ?? 0, fill: "backwards", duration: t.durationMs, easing: `linear(${t.samples.join(", ")})` };
}

export function Motion({ to, from, replay = null, transition, onSettled, style, onLayout, children }: MotionProps) {
  const node = useRef<View | null>(null);
  const running = useRef<Animation | null>(null);
  const stood = useRef<Required<Pose> | null>(null);
  const replayed = useRef(replay);
  const settled = useRef(onSettled);
  settled.current = onSettled;

  const target = resolve(to);

  useLayoutEffect(() => {
    const el = node.current as unknown as HTMLElement | null;
    if (!el) return;

    const replaying = replay !== null && replay !== replayed.current;
    replayed.current = replay;

    let start: Keyframe | null;
    if (stood.current === null || replaying) {
      start = from ? keyframe(resolve(from)) : null;
    } else if (running.current !== null) {
      // Read before the cancel: while running, the animation is what the element computes to.
      const computed = getComputedStyle(el);
      start = { opacity: computed.opacity, transform: computed.transform };
    } else {
      start = same(stood.current, target) ? null : keyframe(stood.current);
    }

    running.current?.cancel();
    running.current = null;
    stood.current = target;

    if (start === null) {
      settled.current?.();
      return;
    }
    const animation = el.animate([start, keyframe(target)], timing(transition));
    running.current = animation;
    animation.finished.then(
      () => {
        if (running.current !== animation) return;
        running.current = null;
        settled.current?.();
      },
      () => {},
    );
  }, [target.opacity, target.translateX, target.translateY, target.scale, target.rotate, replay]);

  // Under StrictMode a view put back must arrive again from `from`; kept as
  // it was, it would read its canceled start as the end and play nothing.
  useLayoutEffect(() => () => {
    running.current?.cancel();
    running.current = null;
    stood.current = null;
  }, []);

  return (
    <View
      ref={node}
      onLayout={onLayout}
      style={[
        style,
        {
          opacity: target.opacity,
          transform: [{ translateX: target.translateX }, { translateY: target.translateY }, { scale: target.scale }, { rotate: `${target.rotate}deg` }],
        },
      ]}
    >
      {children}
    </View>
  );
}

export function Spin({ periodMs, children }: SpinProps) {
  const node = useRef<View | null>(null);
  useLayoutEffect(() => {
    const el = node.current as unknown as HTMLElement | null;
    const turn = el?.animate([{ transform: "rotate(0deg)" }, { transform: "rotate(360deg)" }], { duration: periodMs, iterations: Infinity, easing: "linear" });
    return () => turn?.cancel();
  }, [periodMs]);
  return <View ref={node}>{children}</View>;
}
