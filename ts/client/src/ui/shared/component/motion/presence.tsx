// @pxl/client/ui/shared/component/motion/presence.tsx

/**
 * Keeps a leaving view drawn until its exit motion settles, so that stores and
 * callers remove a thing at once and never wait on an animation.
 */

import { type ReactNode, useMemo, useRef, useState } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
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

export function Presence({
  show,
  from,
  to = {},
  exit,
  transition,
  exitTransition = transition,
  style,
  children,
}: PresenceProps) {
  const [mounted, setMounted] = useState(show);
  if (show && !mounted) setMounted(true);
  if (!show && !mounted) return null;
  return (
    <Motion
      from={from}
      to={show ? to : exit}
      transition={show ? transition : exitTransition}
      onSettled={show ? undefined : () => setMounted(false)}
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
  /** False while the item is leaving. A leaving item is drawn at its exit
   * until `on_exited` drops it. */
  present: boolean;
}

interface Leaving<T> {
  item: T;
  key: Key;
}

/** `items` is compared by identity, so pass the same array until the list changes. */
export function usePresent<T>(
  items: readonly T[],
  key_of: (item: T) => Key,
): { list: Present<T>[]; on_exited: (key: Key) => void } {
  const [prev, setPrev] = useState(items);
  const [leaving, setLeaving] = useState<Leaving<T>[]>([]);

  let current = leaving;
  if (prev !== items) {
    const keys = new Set(items.map(key_of));
    const leaving_keys = new Set(leaving.map((l) => l.key));
    const left = prev
      .map((item) => ({ item, key: key_of(item) }))
      .filter((l) => !keys.has(l.key) && !leaving_keys.has(l.key));
    current = [...leaving.filter((l) => !keys.has(l.key)), ...left];
    setLeaving(current);
    setPrev(items);
  }

  return {
    list: [
      ...items.map((item) => ({ item, key: key_of(item), present: true })),
      ...current.map((l) => ({ ...l, present: false })),
    ],
    on_exited: (key) => setLeaving((l) => l.filter((x) => x.key !== key)),
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

/** Draws the child of `id`. When `id` changes, the old child plays its exit,
 * and the new child enters from `from` only once the old one has gone. The
 * first child drawn does not enter. */
export function Swap({
  id,
  from,
  exit,
  transition,
  exitTransition = transition,
  style,
  children,
}: SwapProps) {
  const ids = useMemo(() => [id], [id]);
  // What each id drew last, so that a leaving id keeps drawing it.
  const nodes = useRef(new Map<Key, ReactNode>());
  nodes.current.set(id, children);
  const [init, setInit] = useState<Key | null>(id);
  if (init !== null && init !== id) setInit(null);
  const { list, on_exited } = usePresent(ids, (k) => k);
  const leaving = list.some((p) => !p.present);
  return list
    .filter((p) => !p.present || !leaving)
    .map(({ key, present }) => (
      <Motion
        key={key}
        from={key === init ? undefined : from}
        to={present ? {} : exit}
        transition={present ? transition : exitTransition}
        onSettled={
          present
            ? undefined
            : () => {
                nodes.current.delete(key);
                on_exited(key);
              }
        }
        style={style}
      >
        {nodes.current.get(key)}
      </Motion>
    ));
}
