// @pxl/web/shared/ui/toast/toaster.tsx

import { useEffect, useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { type Key, usePresent } from "@/shared/ui/motion/presence.tsx";
import { after, LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { setToastDragging, toastDepth, toastRotate, useToastStore } from "./store.ts";
import { Toast } from "./toast.tsx";

const MAX_STACK_DEPTH = 2;
const STACK_PEEK_PX = 8;
const STACK_OPACITY = [1, 0.6, 0.3];
const STACK_SCALE = [1, 0.94, 0.88];

const SWIPE_PX = 24;
const FROM_ABOVE_PX = -24;

/** Pushed-back cards wait half the arrival, so the deck's front never drops before the new card is visible. */
const ARRIVE = SETTLE;
const MAKE_ROOM = after(SETTLE, ARRIVE.durationMs / 2);

export function Toaster() {
  const entries = useToastStore((s) => s.entries);
  const rotation = useToastStore((s) => s.rotation);
  const { list, gone } = usePresent(entries, (e) => e.id);
  // On a whole pixel: half an odd width is not one.
  const [center, setCenter] = useState(0);

  // A leaving card keeps its place until it has faded, so the cards behind
  // move up only once it is gone.
  const lastOrder = useRef<Key[]>([]);
  const lastRotation = useRef(rotation);
  const turnedBack = rotation < lastRotation.current;
  const order: Key[] = entries
    .map((e, i) => ({ key: e.id, depth: toastDepth(i, rotation, entries.length) }))
    .sort((a, b) => a.depth - b.depth)
    .map((c) => c.key);
  const arrivals = new Set<Key>(order.filter((key) => !lastOrder.current.includes(key)));
  const arrived = arrivals.size;
  for (const key of list.filter((c) => !c.present).map((c) => c.key).sort((a, b) => lastOrder.current.indexOf(a) - lastOrder.current.indexOf(b))) {
    order.splice(Math.min(Math.max(0, lastOrder.current.indexOf(key)) + arrived, order.length), 0, key);
  }
  useEffect(() => {
    lastOrder.current = order;
    lastRotation.current = rotation;
  });

  const swipe = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!swipe.current) {
    // Turns once, when the swipe crosses the line; the card does not follow
    // the finger, which would set a pose every move on the kernel's thread.
    let turned = false;
    const read = (dy: number) => {
      if (turned || Math.abs(dy) <= SWIPE_PX) return;
      turned = true;
      toastRotate(dy < 0 ? 1 : -1);
    };
    const end = (dy: number) => {
      read(dy);
      setToastDragging(false);
    };
    swipe.current = PanResponder.create({
      // On press: by the first move, a swipe down has left the pointer to the mode bar.
      onStartShouldSetPanResponder: () => useToastStore.getState().entries.length > 1,
      onPanResponderGrant: () => {
        turned = false;
        setToastDragging(true);
      },
      // A flick can cross the line in the release event alone.
      onPanResponderMove: (_e, g) => read(g.dy),
      onPanResponderRelease: (_e, g) => end(g.dy),
      onPanResponderTerminate: (_e, g) => end(g.dy),
    });
  }

  return (
    <View style={styles.anchor} onLayout={(e) => setCenter(PixelRatio.roundToNearestPixel(e.nativeEvent.layout.width / 2))}>
      {list.map(({ item, key, present }) => {
        const depth = order.indexOf(key);
        const drawn = present && depth <= MAX_STACK_DEPTH;
        const front = present && depth === 0;
        const fromAbove = front && turnedBack && lastOrder.current.indexOf(key) !== 0;
        const rest = { translateY: depth * STACK_PEEK_PX, scale: STACK_SCALE[Math.min(depth, MAX_STACK_DEPTH)] };
        const transition = !present ? LEAVE : arrivals.has(key) ? ARRIVE : arrived > 0 ? MAKE_ROOM : SETTLE;
        // No opacity here: it would hide a device's glass wrongly.
        return (
          <Motion
            key={key}
            from={fromAbove ? { translateY: FROM_ABOVE_PX } : rest}
            replay={fromAbove ? rotation : null}
            to={present ? rest : { scale: 0.9 }}
            transition={transition}
            onSettled={present ? undefined : () => gone(key)}
            style={[styles.card, { left: center, zIndex: MAX_STACK_DEPTH + 1 - Math.min(depth, MAX_STACK_DEPTH + 1), pointerEvents: front ? "auto" : "none" }]}
          >
            <Toast
              message={item.message}
              tone={item.tone}
              fade={drawn ? STACK_OPACITY[depth] : 0}
              fadeTransition={transition}
              touch={front ? swipe.current!.panHandlers : undefined}
            />
          </Motion>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  // No card is placed by measuring another.
  anchor: { alignItems: "center", height: 32, pointerEvents: "box-none" },
  card: { position: "absolute", bottom: 0, width: 0, height: 32, transformOrigin: "center bottom" },
});
