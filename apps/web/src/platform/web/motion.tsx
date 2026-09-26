// @pxl/web/platform/web/motion.tsx

/**
 * A view that moves to the pose it is given, on the page: the browser's own
 * animation of the element (Web Animations), which runs opacity and a
 * transform on the compositor. The script's part is one call per new pose;
 * no frame of it runs on the thread the kernel spends.
 *
 * The committed style is always the pose given, and an animation only plays
 * over it, so whatever interrupts or cancels one leaves the view where it was
 * told to be. A pose that arrives mid-flight starts from where the running
 * one has it, read off the element, so a turn back never jumps.
 */

import { useEffect, useLayoutEffect, useRef } from "react";
import { View } from "react-native";

import { type MotionProps, type Pose, resolve, type SpinProps } from "@/shared/ui/motion/pose.ts";
import { springCurve, type Transition } from "@/shared/ui/motion/transition.ts";

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
  // `backwards` holds the start through a delay; after the end the committed
  // style, which is the end, shows by itself.
  const base = { delay: t.delayMs ?? 0, fill: "backwards" as const };
  if (t.type === "ease") return { ...base, duration: t.durationMs, easing: `cubic-bezier(${t.bezier.join(", ")})` };
  const curve = springCurve(t);
  return { ...base, duration: curve.durationMs, easing: `linear(${curve.samples.join(", ")})` };
}

export function Motion({ to, from, replay = null, transition, onSettled, style, onLayout, children }: MotionProps) {
  const node = useRef<View | null>(null);
  const running = useRef<Animation | null>(null);
  /** The pose last committed; null before the first. */
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
      // Read before the cancel below: while it runs, the animation is what
      // the element computes to, not its committed style.
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
      // Canceled: whatever canceled it has taken over.
      () => {},
    );
    // `transition` and `from` are read when a pose arrives, not watched: a
    // new transition alone moves nothing.
  }, [target.opacity, target.translateX, target.translateY, target.scale, target.rotate, replay]);

  useEffect(() => () => running.current?.cancel(), []);

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

/** A view turning round and round, once a period, at an even rate: a load
 * that has not said how far along it is. The browser's animation, on the
 * compositor, for as long as it is mounted. */
export function Spin({ periodMs, children }: SpinProps) {
  const node = useRef<View | null>(null);
  useLayoutEffect(() => {
    const el = node.current as unknown as HTMLElement | null;
    const turn = el?.animate([{ transform: "rotate(0deg)" }, { transform: "rotate(360deg)" }], { duration: periodMs, iterations: Infinity, easing: "linear" });
    return () => turn?.cancel();
  }, [periodMs]);
  return <View ref={node}>{children}</View>;
}
