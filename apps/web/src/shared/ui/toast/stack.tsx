// @pxl/web/shared/ui/toast/stack.tsx

import { useEffect, useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { cardPose, deckSwipe, peekBelow, useTurnedBack } from "@/shared/ui/deck.ts";
import { type Key, usePresent } from "@/shared/ui/motion/presence.tsx";
import { after, LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { setToastDragging, toastRotate, useToastStore } from "./store.ts";
import { Toast, TOAST_SIDE } from "./item.tsx";

const PEEK_PX = 8;

/** Pushed-back cards wait half the arrival, so the deck's front never drops before the new card is visible. */
const MAKE_ROOM = after(SETTLE, SETTLE.durationMs / 2);

export function Toaster() {
  const entries = useToastStore((s) => s.entries);
  const turns = useToastStore((s) => s.turns);
  const { list, gone } = usePresent(entries, (e) => e.id);
  // On a whole pixel: half an odd width is not one.
  const [center, setCenter] = useState(0);

  // A leaving card keeps its place until it has faded, so the cards behind
  // move up only once it is gone.
  const lastOrder = useRef<Key[]>([]);
  const turnedBack = useTurnedBack(turns);
  const order: Key[] = entries.map((e) => e.id);
  const arrivals = new Set<Key>(order.filter((key) => !lastOrder.current.includes(key)));
  const arrived = arrivals.size;
  for (const key of list.filter((c) => !c.present).map((c) => c.key).sort((a, b) => lastOrder.current.indexOf(a) - lastOrder.current.indexOf(b))) {
    order.splice(Math.min(Math.max(0, lastOrder.current.indexOf(key)) + arrived, order.length), 0, key);
  }
  useEffect(() => {
    lastOrder.current = order;
  });

  const swipe = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!swipe.current) {
    swipe.current = PanResponder.create({
      // On press: by the first move, a swipe down has left the pointer to the dock.
      onStartShouldSetPanResponder: () => useToastStore.getState().entries.length > 1,
      ...deckSwipe(toastRotate, { drag: setToastDragging }),
    });
  }

  return (
    <View style={styles.anchor} onLayout={(e) => setCenter(PixelRatio.roundToNearestPixel(e.nativeEvent.layout.width / 2))}>
      {
        // In a fixed order, stacked by zIndex alone: a card React moves in the tree drops its running animation.
      }
      {[...list].sort((a, b) => (a.key as number) - (b.key as number)).map(({ item, key, present }) => {
        const depth = order.indexOf(key);
        const pose = cardPose(depth, PEEK_PX);
        const front = present && depth === 0;
        const fromAbove = front && turnedBack && lastOrder.current.indexOf(key) !== 0;
        const transition = !present ? LEAVE : arrivals.has(key) ? SETTLE : arrived > 0 ? MAKE_ROOM : SETTLE;
        // No opacity here: it would hide a device's glass wrongly.
        return (
          <Motion
            key={key}
            from={fromAbove ? pose.above : pose.rest}
            replay={fromAbove ? turns : null}
            to={present ? pose.rest : { scale: 0.9 }}
            transition={transition}
            onSettled={present ? undefined : () => gone(key)}
            style={[styles.card, { left: center, zIndex: pose.zIndex, pointerEvents: front ? "auto" : "none" }]}
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
  // No card is placed by measuring another.
  anchor: { alignItems: "center", height: TOAST_SIDE, pointerEvents: "box-none" },
  card: { position: "absolute", bottom: 0, width: 0, height: TOAST_SIDE, transformOrigin: "center bottom" },
});
