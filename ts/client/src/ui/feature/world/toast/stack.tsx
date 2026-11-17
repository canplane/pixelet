// @pxl/client/ui/feature/world/toast/stack.tsx

import { useEffect, useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import {
  card_pose,
  deck_swipe,
  peek_below,
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
  rotate_toasts,
  set_toast_dragging,
  toast_count,
  useToastEntries,
  useToastTurnCount,
} from "./store.ts";
import { Toast, TOAST_SIDE } from "./item.tsx";

const PEEK_PX = 8;

/** The cards that a new card pushes back wait for half of its arrival before
 * they move, so that the front of the deck never empties before the new card
 * shows. */
const MAKE_ROOM = after(SETTLE, SETTLE.duration_ms / 2);

export function Toaster() {
  const entries = useToastEntries();
  const turn_count = useToastTurnCount();
  const { list, on_exited } = usePresent(entries, (e) => e.id);
  // The center is rounded to a whole pixel, because half of an odd width is
  // not one.
  const [center, set_center] = useState(0);

  // A leaving card keeps its place until it has faded, so the cards behind
  // move up only once it is gone.
  const last_order = useRef<Key[]>([]);
  const turned_back = useTurnedBack(turn_count);
  const order: Key[] = entries.map((e) => e.id);
  const arrivals = new Set<Key>(
    order.filter((key) => !last_order.current.includes(key)),
  );
  const arrived = arrivals.size;
  // A leaving card stays behind the card it was behind, and behind any card
  // that has just come in there. A new card goes in by rank, so it can land
  // anywhere in the deck, not only at the front.
  for (const key of list
    .filter((c) => !c.present)
    .map((c) => c.key)
    .sort(
      (a, b) => last_order.current.indexOf(a) - last_order.current.indexOf(b),
    )) {
    const ahead = last_order.current
      .slice(0, Math.max(0, last_order.current.indexOf(key)))
      .findLast((k) => order.includes(k));
    let at = ahead === undefined ? 0 : order.indexOf(ahead) + 1;
    while (at < order.length && arrivals.has(order[at])) at++;
    order.splice(at, 0, key);
  }
  useEffect(() => {
    last_order.current = order;
  });

  const swipe = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!swipe.current) {
    swipe.current = PanResponder.create({
      // The stack claims a touch as it starts. A mouse drag on the web reports
      // each move to whatever lies under the cursor, so a swipe down that
      // reached the dock before the stack claimed it would turn the dock.
      onStartShouldSetPanResponder: () => toast_count() > 1,
      ...deck_swipe(rotate_toasts, { drag: set_toast_dragging }),
    });
  }

  return (
    <View
      style={STYLE.anchor}
      onLayout={(e) =>
        set_center(
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
          const pose = card_pose(depth, PEEK_PX);
          const front = present && depth === 0;
          const from_above =
            front && turned_back && last_order.current.indexOf(key) !== 0;
          const transition = !present
            ? LEAVE
            : arrivals.has(key)
              ? SETTLE
              : arrived > 0
                ? MAKE_ROOM
                : SETTLE;
          // This view takes no opacity, because an opacity over the native
          // glass draws the glass wrongly. The card fades through `fade`.
          return (
            <Motion
              key={key}
              from={from_above ? pose.above : pose.rest}
              replay={from_above ? turn_count : null}
              to={present ? pose.rest : { scale: 0.9 }}
              transition={transition}
              onSettled={present ? undefined : () => on_exited(key)}
              style={[
                STYLE.card,
                {
                  left: center,
                  zIndex: pose.z_index,
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
                touchBelow={front ? peek_below(entries.length, PEEK_PX) : 0}
              />
            </Motion>
          );
        })}
    </View>
  );
}

const STYLE = StyleSheet.create({
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
