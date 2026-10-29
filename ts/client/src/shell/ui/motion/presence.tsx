// @pxl/client/shell/ui/motion/presence.tsx

/**
 * Keeps a leaving view rendered until its exit motion settles. Stores and
 * callers remove things at once and never wait on an animation.
 */

import { type ReactNode, useMemo, useRef, useState } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import type { Pose } from "./pose.ts";
import type { Transition } from "./transition.ts";

interface PresenceProps {
  show: boolean;
  from: Pose;
  to?: Pose;
  exit: Pose;
  transition: Transition;
  exitTransition?: Transition;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

export function Presence(
  {
    show,
    from,
    to = {},
    exit,
    transition,
    exitTransition = transition,
    style,
    children,
  }: PresenceProps,
) {
  const [held, setHeld] = useState(show);
  if (show && !held) setHeld(true);
  if (!show && !held) return null;
  return (
    <Motion
      from={from}
      to={show ? to : exit}
      transition={show ? transition : exitTransition}
      onSettled={show ? undefined : () => setHeld(false)}
      style={style}
    >
      {children}
    </Motion>
  );
}

export type Key = string | number;

interface Present<T> {
  item: T;
  key: Key;
  /** False while it is leaving: drawn at its exit, and dropped by `gone`. */
  present: boolean;
}

interface Leaving<T> {
  item: T;
  key: Key;
}

/** `items` is compared by identity, so pass the same array until the list changes. */
export function usePresent<T>(
  items: readonly T[],
  keyOf: (item: T) => Key,
): { list: Present<T>[]; gone: (key: Key) => void } {
  const [prev, setPrev] = useState(items);
  const [leaving, setLeaving] = useState<Leaving<T>[]>([]);

  let current = leaving;
  if (prev !== items) {
    const asked = new Set(items.map(keyOf));
    const held = new Set(leaving.map((l) => l.key));
    const left = prev.map((item) => ({ item, key: keyOf(item) })).filter((l) =>
      !asked.has(l.key) && !held.has(l.key)
    );
    current = [...leaving.filter((l) => !asked.has(l.key)), ...left];
    setLeaving(current);
    setPrev(items);
  }

  return {
    list: [
      ...items.map((item) => ({ item, key: keyOf(item), present: true })),
      ...current.map((l) => ({ ...l, present: false })),
    ],
    gone: (key) => setLeaving((l) => l.filter((x) => x.key !== key)),
  };
}

interface SwapProps {
  id: Key;
  from: Pose;
  exit: Pose;
  transition: Transition;
  exitTransition?: Transition;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

export function Swap(
  { id, from, exit, transition, exitTransition = transition, style, children }:
    SwapProps,
) {
  const ids = useMemo(() => [id], [id]);
  // What each id last drew, so a leaving one keeps drawing it.
  const nodes = useRef(new Map<Key, ReactNode>());
  nodes.current.set(id, children);
  const [init, setInit] = useState<Key | null>(id);
  if (init !== null && init !== id) setInit(null);
  const { list, gone } = usePresent(ids, (k) => k);
  const leaving = list.some((p) => !p.present);
  return list.filter((p) => !p.present || !leaving).map(({ key, present }) => (
    <Motion
      key={key}
      from={key === init ? undefined : from}
      to={present ? {} : exit}
      transition={present ? transition : exitTransition}
      onSettled={present ? undefined : () => {
        nodes.current.delete(key);
        gone(key);
      }}
      style={style}
    >
      {nodes.current.get(key)}
    </Motion>
  ));
}
