// @pxl/client/ui/feature/world/deck.ts

/** A deck of cards stacked front to back and turned by a vertical swipe. The
 * toasts and the dock are both decks. */

import { useEffect, useRef } from "react";
import { type PanGesture, usePanGesture } from "react-native-gesture-handler";

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";

const DEPTH = 2;
/** Where a card brought from the back to the front starts, counted in peeks
 * from its rest. A negative count is above it. */
const FROM_ABOVE = -3;
const OPACITY = [1, 0.6, 0.3];
const SCALE = [1, 0.94, 0.88];
const SWIPE_PX = 24;
/** How far a press may move and still be a press, not a drag. */
export const SLOP_PX = 12;

interface CardPose {
  rest: { translateY: f64; scale: f64 };
  above: { translateY: f64 };
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

/** How far the cards behind show under the front one. The front card's touch
 * area reaches over them too. */
export function peekBelow(count: u32, peek: f64): f64 {
  return Math.min(Math.max(count - 1, 0), DEPTH) * peek;
}

/** Whether the last turn brought the back card forward. Such a card comes in
 * from above. */
export function useTurnedBack(rotation: i32): boolean {
  const last = useRef(rotation);
  const turnedBack = rotation < last.current;
  useEffect(() => {
    last.current = rotation;
  });
  return turnedBack;
}

/**
 * A deck's swipe. A swipe up turns the front card back, and a swipe down
 * brings the back one forward. The deck turns once, when the hand has gone
 * `SWIPE_PX` up or down, and not at all once it has gone `SLOP_PX` sideways
 * first. The card does not follow the finger. `drag` is told while a swipe is
 * under way.
 */
export function useDeckSwipe(
  turn: (delta: i32) => void,
  { enabled = true, drag }: { enabled?: boolean; drag?: (on: boolean) => void },
): PanGesture {
  return usePanGesture({
    enabled,
    activeOffsetY: [-SWIPE_PX, SWIPE_PX],
    failOffsetX: [-SLOP_PX, SLOP_PX],
    onActivate: (e) => {
      turn(e.velocityY < 0 ? 1 : -1);
      drag?.(true);
    },
    onDeactivate: () => drag?.(false),
  });
}
