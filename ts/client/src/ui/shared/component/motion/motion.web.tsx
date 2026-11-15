// @pxl/client/ui/shared/component/motion/motion.web.tsx

/**
 * A view on a page, moved by the Web Animations API, which runs on the
 * compositor. The view's style is always the pose it was given last, so a
 * canceled animation leaves the view where it was told to be. A pose given
 * while an animation runs starts from the view's computed style.
 */

import { useLayoutEffect, useRef } from "react";
import { View, type ViewInstance } from "react-native";

import {
  CHANNELS,
  type MotionProps,
  type Pose,
  resolve,
  type SpinProps,
  useReplay,
} from "@/ui/shared/component/motion/pose.ts";
import type { Transition } from "@/ui/shared/component/motion/transition.ts";

function keyframe(p: Required<Pose>): Keyframe {
  return {
    opacity: String(p.opacity),
    transform: `translateX(${p.translateX}px) translateY(${p.translateY}px) scale(${p.scale}) rotate(${p.rotate}deg)`,
  };
}

function same(a: Required<Pose>, b: Required<Pose>): boolean {
  return CHANNELS.every((c) => a[c] === b[c]);
}

function timing(t: Transition): KeyframeAnimationOptions {
  return {
    delay: t.delay_ms ?? 0,
    fill: "backwards",
    duration: t.duration_ms,
    easing: `linear(${t.samples.join(", ")})`,
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
  const node = useRef<ViewInstance | null>(null);
  const running = useRef<Animation | null>(null);
  const rest = useRef<Required<Pose> | null>(null);
  const replays = useReplay(replay);
  const settled = useRef(onSettled);
  settled.current = onSettled;

  const target = resolve(to);

  useLayoutEffect(() => {
    const el = node.current as unknown as HTMLElement | null;
    if (!el) return;

    const replaying = replays();

    let start: Keyframe | null;
    if (rest.current === null || replaying) {
      start = from ? keyframe(resolve(from)) : null;
    } else if (running.current !== null) {
      // The computed style is read before the cancel, because only while the
      // animation runs does the element compute to where it is in flight.
      const computed = getComputedStyle(el);
      start = { opacity: computed.opacity, transform: computed.transform };
    } else {
      start = same(rest.current, target) ? null : keyframe(rest.current);
    }

    running.current?.cancel();
    running.current = null;
    rest.current = target;

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
  }, [...CHANNELS.map((c) => target[c]), replay]);

  // Under StrictMode, React takes a view down and puts it back. The view put
  // back must arrive from `from` again: with its old state kept, it would take
  // its canceled start for its end and play nothing.
  useLayoutEffect(
    () => () => {
      running.current?.cancel();
      running.current = null;
      rest.current = null;
    },
    [],
  );

  return (
    <View
      ref={node}
      onLayout={onLayout}
      style={[
        style,
        {
          opacity: target.opacity,
          transform: [
            { translateX: target.translateX },
            { translateY: target.translateY },
            { scale: target.scale },
            { rotate: `${target.rotate}deg` },
          ],
        },
      ]}
    >
      {children}
    </View>
  );
}

export function Spin({ periodMs, children }: SpinProps) {
  const node = useRef<ViewInstance | null>(null);
  useLayoutEffect(() => {
    const el = node.current as unknown as HTMLElement | null;
    const turn = el?.animate(
      [
        { transform: "rotate(0deg)" },
        {
          transform: "rotate(360deg)",
        },
      ],
      { duration: periodMs, iterations: Infinity, easing: "linear" },
    );
    return () => turn?.cancel();
  }, [periodMs]);
  return <View ref={node}>{children}</View>;
}
