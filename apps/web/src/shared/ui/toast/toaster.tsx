// @pxl/web/shared/ui/toast/toaster.tsx
// The deck owns its layout end to end rather than taking a toast library's: a
// library stacks each toast by measuring its neighbors and dismisses it with a
// swipe, where these cards all stand at one point and a swipe turns them.

import { useEffect, useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { type Key, usePresent } from "@/shared/ui/motion/presence.tsx";
import { after, LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { setToastDragging, toastDepth, toastRotate, useToastStore } from "./store.ts";
import { Toast } from "./toast.tsx";

/** Cards drawn behind the front one; one queued further back is kept in the
 * deck, and comes forward as the deck turns, but is not drawn. */
const MAX_STACK_DEPTH = 2;
const STACK_PEEK_PX = 6;
const STACK_OPACITY = [1, 0.6, 0.3];
const STACK_SCALE = [1, 0.94, 0.88];

/** A swipe this far up or down turns the deck a card. */
const SWIPE_PX = 24;
/** Where a card comes from when a swipe down brings it to the front: above
 * the deck, as if it had been on top of the others all along. */
const FROM_ABOVE_PX = -24;

/** A new card fades in as a card settles, and the cards it pushes back wait
 * half of it: a card stays where it stands until one mostly covers it, so
 * the front of the deck never drops and rises again while the new one is
 * still too faint to be seen over it. */
const ARRIVE = SETTLE;
const MAKE_ROOM = after(SETTLE, ARRIVE.durationMs / 2);

/**
 * The deck: every toast at one point, the front card on top and the next
 * two peeking out under it. Swiping the front card up or down turns the deck.
 */
export function Toaster() {
  const entries = useToastStore((s) => s.entries);
  const rotation = useToastStore((s) => s.rotation);
  const { list, gone } = usePresent(entries, (e) => e.id);
  // The deck's point, on a whole pixel: half an odd width is not one.
  const [center, setCenter] = useState(0);

  // Where each card stands, front first: the deck as the store turns it, and
  // a card leaving kept at the place it had until it has faded, so the cards
  // behind it hold theirs and move up once it is gone, not while it goes.
  // Both that place and which way the deck last turned are read against the
  // last commit, so they are written after it: a card that a turn back
  // brings to the front comes down from above, every other card moves
  // between depths in place.
  const lastOrder = useRef<Key[]>([]);
  const lastRotation = useRef(rotation);
  const turnedBack = rotation < lastRotation.current;
  const order: Key[] = entries
    .map((e, i) => ({ key: e.id, depth: toastDepth(i, rotation, entries.length) }))
    .sort((a, b) => a.depth - b.depth)
    .map((c) => c.key);
  // A card that has just arrived is in front of every card already there,
  // a leaving one included.
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
    // The deck turns the moment a swipe crosses the line, not on release,
    // and once a swipe: holding past it does not keep turning. Only the
    // crossing is read; the card does not follow the finger, which would be
    // a pose set every move on the thread the kernel spends.
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
      // Claimed on the press, not on the first move: a pointer's move is
      // offered to whatever it is over by then, which a swipe down has left
      // for the mode bar. A card has no tap of its own to give up for it.
      onStartShouldSetPanResponder: () => useToastStore.getState().entries.length > 1,
      onPanResponderGrant: () => {
        turned = false;
        setToastDragging(true);
      },
      // Every report of the finger is read the same way, the one it lifted
      // on included: a flick can cross the line inside it, with no move
      // reported between.
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
        // How present the card is is its pill's to show (shared/ui/motion/veil.ts):
        // an opacity laid over it here would hide a device's glass wrongly.
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
  // Every card stands on this one point; no card is placed by measuring
  // another, which a card arriving, leaving or changing width would upset.
  anchor: { alignItems: "center", height: 32, pointerEvents: "box-none" },
  // Nothing wide itself: the card's pill centers itself on this point, and
  // the deck's scale is about the point's foot.
  card: { position: "absolute", bottom: 0, width: 0, height: 32, transformOrigin: "center bottom" },
});
