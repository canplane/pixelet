// @pxl/client/ui/feature/world/toast/stack.tsx

import { useEffect, useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import {
  cardPose,
  deckSwipe,
  peekBelow,
  useTurnedBack,
} from "@/ui/feature/world/deck.ts";
import {
  type Key,
  usePresent,
} from "@/ui/shared/component/motion/presence.tsx";
import {
  after,
  LEAVE,
  SETTLE,
} from "@/ui/shared/component/motion/transition.ts";
import {
  rotateToasts,
  setToastDragging,
  toastCount,
  useToastEntries,
  useToastTurns,
} from "./store.ts";
import { Toast, TOAST_SIDE } from "./item.tsx";

const PEEK_PX = 8;

/** The cards that a new card pushes back wait for half of its arrival before
 * they move, so that the front of the deck never empties before the new card
 * shows. */
const MAKE_ROOM = after(SETTLE, SETTLE.durationMs / 2);

export function Toaster() {
  const entries = useToastEntries();
  const turns = useToastTurns();
  const { list, onExited } = usePresent(entries, (e) => e.id);
  // The center is rounded to a whole pixel, because half of an odd width is
  // not one.
  const [center, setCenter] = useState(0);

  // A leaving card keeps its place until it has faded, so the cards behind
  // move up only once it is gone.
  const lastOrder = useRef<Key[]>([]);
  const turnedBack = useTurnedBack(turns);
  const order: Key[] = entries.map((e) => e.id);
  const arrivals = new Set<Key>(
    order.filter((key) => !lastOrder.current.includes(key)),
  );
  const arrived = arrivals.size;
  // A leaving card stays behind the card it was behind, and behind any card
  // that has just come in there. A new card goes in by rank, so it can land
  // anywhere in the deck, not only at the front.
  for (const key of list
    .filter((c) => !c.present)
    .map((c) => c.key)
    .sort(
      (a, b) => lastOrder.current.indexOf(a) - lastOrder.current.indexOf(b),
    )) {
    const ahead = lastOrder.current
      .slice(0, Math.max(0, lastOrder.current.indexOf(key)))
      .findLast((k) => order.includes(k));
    let at = ahead === undefined ? 0 : order.indexOf(ahead) + 1;
    while (at < order.length && arrivals.has(order[at])) at++;
    order.splice(at, 0, key);
  }
  useEffect(() => {
    lastOrder.current = order;
  });

  const swipe = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!swipe.current) {
    swipe.current = PanResponder.create({
      // The stack claims a touch as it starts. A mouse drag on a page reports
      // each move to whatever lies under the cursor, so a swipe down that
      // reached the dock before the stack claimed it would turn the dock.
      onStartShouldSetPanResponder: () => toastCount() > 1,
      ...deckSwipe(rotateToasts, { drag: setToastDragging }),
    });
  }

  return (
    <View
      style={styles.anchor}
      onLayout={(e) =>
        setCenter(
          PixelRatio.roundToNearestPixel(e.nativeEvent.layout.width / 2),
        )
      }
    >
      {
        // The cards render in a fixed order and stack by zIndex alone,
        // because a card that React moves within the tree drops its running
        // animation.
      }
      {[...list]
        .sort((a, b) => (a.key as number) - (b.key as number))
        .map(({ item, key, present }) => {
          const depth = order.indexOf(key);
          const pose = cardPose(depth, PEEK_PX);
          const front = present && depth === 0;
          const fromAbove =
            front && turnedBack && lastOrder.current.indexOf(key) !== 0;
          const transition = !present
            ? LEAVE
            : arrivals.has(key)
              ? SETTLE
              : arrived > 0
                ? MAKE_ROOM
                : SETTLE;
          // This view takes no opacity, because an opacity over a device's
          // glass draws the glass wrongly. The card fades through `fade`.
          return (
            <Motion
              key={key}
              from={fromAbove ? pose.above : pose.rest}
              replay={fromAbove ? turns : null}
              to={present ? pose.rest : { scale: 0.9 }}
              transition={transition}
              onSettled={present ? undefined : () => onExited(key)}
              style={[
                styles.card,
                {
                  left: center,
                  zIndex: pose.zIndex,
                  pointerEvents: front ? "auto" : "none",
                },
              ]}
            >
              <Toast
                message={item.message}
                tone={item.tone}
                fade={present ? pose.fade : 0}
                fadeTransition={transition}
                touch={front ? swipe.current!.panHandlers : undefined}
                touchBelow={front ? peekBelow(entries.length, PEEK_PX) : 0}
              />
            </Motion>
          );
        })}
    </View>
  );
}

const styles = StyleSheet.create({
  // Every card is placed from this anchor, and none by measuring another.
  anchor: {
    alignItems: "center",
    height: TOAST_SIDE,
    pointerEvents: "box-none",
  },
  card: {
    position: "absolute",
    bottom: 0,
    width: 0,
    height: TOAST_SIDE,
    transformOrigin: "center bottom",
  },
});
