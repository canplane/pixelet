// @pxl/web/shared/ui/motion/presence.tsx

/**
 * What leaves is kept until it has left. React drops a view the render it
 * stops being asked for, so a view that is to fade or slide away has to stay
 * rendered, told to go, until its motion settles — and only then be dropped.
 * The leaving is the view's alone: a store or a caller that removes a thing
 * removes it at once, and never waits on an animation.
 */

import { type ReactNode, useMemo, useRef, useState } from "react";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@platform/motion.tsx";
import type { Pose } from "./pose.ts";
import type { Transition } from "./transition.ts";

interface PresenceProps {
  show: boolean;
  /** Where it comes in from. */
  from: Pose;
  /** Where it stands while shown; at rest unless said. */
  to?: Pose;
  /** Where it goes before it is dropped. */
  exit: Pose;
  transition: Transition;
  /** Leaving, where it differs from arriving. */
  exitTransition?: Transition;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

/** One view that comes and goes with `show`. */
export function Presence({ show, from, to = {}, exit, transition, exitTransition = transition, style, children }: PresenceProps) {
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

export interface Present<T> {
  item: T;
  key: string;
  /** False while it is leaving: drawn at its exit, and dropped by `gone`. */
  present: boolean;
}

interface Leaving<T> {
  item: T;
  key: string;
}

/**
 * A list whose members leave rather than vanish: every item asked for, then
 * every item that has stopped being asked for and has not yet called
 * `gone`, as it last was. An item asked for again while leaving comes back
 * as itself. `items` is compared by identity, so a caller hands the same
 * array until the list changes.
 */
export function usePresent<T>(items: readonly T[], keyOf: (item: T) => string): { list: Present<T>[]; gone: (key: string) => void } {
  const [prev, setPrev] = useState(items);
  const [leaving, setLeaving] = useState<Leaving<T>[]>([]);

  let current = leaving;
  if (prev !== items) {
    const asked = new Set(items.map(keyOf));
    const held = new Set(leaving.map((l) => l.key));
    const left = prev.map((item) => ({ item, key: keyOf(item) })).filter((l) => !asked.has(l.key) && !held.has(l.key));
    current = [...leaving.filter((l) => !asked.has(l.key)), ...left];
    setLeaving(current);
    setPrev(items);
  }

  return {
    list: [...items.map((item) => ({ item, key: keyOf(item), present: true })), ...current.map((l) => ({ ...l, present: false }))],
    gone: (key) => setLeaving((l) => l.filter((x) => x.key !== key)),
  };
}

interface SwapProps {
  /** What is shown; a new id's view comes in once the old one has left. */
  id: string;
  from: Pose;
  exit: Pose;
  transition: Transition;
  exitTransition?: Transition;
  /** Where each stands, the leaving one and the next in turn. */
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

/** One view at a time: the last one leaves, then the next arrives. What it
 * mounts with is simply there; only a change comes and goes. */
export function Swap({ id, from, exit, transition, exitTransition = transition, style, children }: SwapProps) {
  const ids = useMemo(() => [id], [id]);
  // What each id last drew, so one leaving keeps drawing it. Written in
  // render, and the same on a second render of the same props.
  const nodes = useRef(new Map<string, ReactNode>());
  nodes.current.set(id, children);
  const [initial, setInitial] = useState<string | null>(id);
  if (initial !== null && initial !== id) setInitial(null);
  const { list, gone } = usePresent(ids, (k) => k);
  const leaving = list.some((p) => !p.present);
  return list.filter((p) => !p.present || !leaving).map(({ key, present }) => (
    <Motion
      key={key}
      from={key === initial ? undefined : from}
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
