// @pxl/client/shared/ui/deck.ts

/** Cards stacked front to back and turned by a vertical swipe: the toasts and the dock. */

import { useEffect, useRef } from "react";
import type { PanResponderCallbacks } from "react-native";

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";

const DEPTH = 2;
/** In peeks: where a card turned back to the front starts. */
const FROM_ABOVE = -3;
const OPACITY = [1, 0.6, 0.3];
const SCALE = [1, 0.94, 0.88];
const SWIPE_PX = 24;

/** The depth of the card at index `i`, front 0, with the deck turned by `rotation`. */
export function deckDepth(i: u32, rotation: i32, count: u32): u32 {
  return count > 0 ? ((i - rotation) % count + count) % count : 0;
}

interface CardPose {
  rest: { translateY: f64; scale: f64 };
  above: { translateY: f64 };
  /** 0 past the cards drawn. */
  fade: f64;
  zIndex: u32;
}

/** `peek` is how far each card behind shows past the one in front of it. */
export function cardPose(depth: u32, peek: f64): CardPose {
  const shown = Math.min(depth, DEPTH);
  return {
    rest: { translateY: depth * peek, scale: SCALE[shown] },
    above: { translateY: peek * FROM_ABOVE },
    fade: depth <= DEPTH ? OPACITY[depth] : 0,
    zIndex: DEPTH + 1 - Math.min(depth, DEPTH + 1),
  };
}

/** How far the cards behind show under the front one, which the front one's touch covers. */
export function peekBelow(count: u32, peek: f64): f64 {
  return Math.min(Math.max(count - 1, 0), DEPTH) * peek;
}

/** Whether the last turn brought the back card forward, so the new front comes from above. */
export function useTurnedBack(rotation: i32): boolean {
  const last = useRef(rotation);
  const turnedBack = rotation < last.current;
  useEffect(() => {
    last.current = rotation;
  });
  return turnedBack;
}

interface SwipeHooks {
  /** At the touch's start, before anything claims it. */
  press?: () => void;
  drag?: (on: boolean) => void;
}

/**
 * A swipe's callbacks: up turns the front card back, down brings the back one
 * forward. It turns once, when the swipe crosses the line; the card does not
 * follow the finger, which would set a pose every move on the kernel's thread.
 */
export function deckSwipe(turn: (delta: i32) => void, { press, drag }: SwipeHooks = {}): PanResponderCallbacks {
  // From where the touch began: a grant after the first moves restarts the gesture's own dy.
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
    // A flick can cross the line in the release event alone.
    onPanResponderMove: (e) => read(e.nativeEvent.pageY - from),
    onPanResponderRelease: (e) => end(e.nativeEvent.pageY - from),
    onPanResponderTerminate: (e) => end(e.nativeEvent.pageY - from),
  };
}
