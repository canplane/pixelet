// @pxl/client/ui/feature/world/deck.ts

/** A deck of cards stacked front to back and turned by a vertical swipe. The
 * toasts and the dock are both decks. */

import { useEffect, useRef } from "react";
import type { PanResponderCallbacks } from "react-native";

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";

const DEPTH = 2;
/** Where a card brought from the back to the front starts, counted in peeks
 * from its rest. A negative count is above it. */
const FROM_ABOVE = -3;
const OPACITY = [1, 0.6, 0.3];
const SCALE = [1, 0.94, 0.88];
const SWIPE_PX = 24;

interface CardPose {
  rest: { translateY: f64; scale: f64 };
  above: { translateY: f64 };
  fade: f64;
  z_index: u32;
}

/** `peek` is how far each card behind shows past the one in front of it. */
export function card_pose(depth: u32, peek: f64): CardPose {
  const shown = Math.min(depth, DEPTH);
  return {
    rest: { translateY: depth * peek, scale: SCALE[shown] },
    above: { translateY: peek * FROM_ABOVE },
    fade: depth <= DEPTH ? OPACITY[depth] : 0,
    z_index: DEPTH + 1 - Math.min(depth, DEPTH + 1),
  };
}

/** How far the cards behind show under the front one. The front card's touch
 * area reaches over them too. */
export function peek_below(count: u32, peek: f64): f64 {
  return Math.min(Math.max(count - 1, 0), DEPTH) * peek;
}

/** Whether the last turn brought the back card forward. Such a card comes in
 * from above. */
export function useTurnedBack(rotation: i32): boolean {
  const last = useRef(rotation);
  const turned_back = rotation < last.current;
  useEffect(() => {
    last.current = rotation;
  });
  return turned_back;
}

interface SwipeHandler {
  /** Called when a touch starts, before any responder claims it. */
  press?: () => void;
  drag?: (on: boolean) => void;
}

/**
 * A swipe's callbacks. A swipe up turns the front card back, and a swipe down
 * brings the back one forward. The deck turns once, when the swipe crosses
 * `SWIPE_PX`. The card does not follow the finger: that would set a pose on
 * every move, and on a device the UI shares its thread with the core.
 */
export function deck_swipe(
  turn: (delta: i32) => void,
  { press, drag }: SwipeHandler = {},
): PanResponderCallbacks {
  // The distance is measured from where the touch began. The responder's own
  // `dy` restarts at the grant, which can come after the first moves.
  let from: f64 = 0;
  let turned = false;
  const read = (dy: f64) => {
    if (turned || Math.abs(dy) <= SWIPE_PX) return;
    turned = true;
    turn(dy < 0 ? 1 : -1);
  };
  const end = (dy: f64) => {
    read(dy);
    drag?.(false);
  };
  return {
    onStartShouldSetPanResponderCapture: (e) => {
      from = e.nativeEvent.pageY;
      press?.();
      return false;
    },
    onPanResponderGrant: () => {
      turned = false;
      drag?.(true);
    },
    // A flick can cross the line in its release event alone, so the release
    // reads the distance too.
    onPanResponderMove: (e) => read(e.nativeEvent.pageY - from),
    onPanResponderRelease: (e) => end(e.nativeEvent.pageY - from),
    onPanResponderTerminate: (e) => end(e.nativeEvent.pageY - from),
  };
}
