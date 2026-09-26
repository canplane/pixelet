// @pxl/web/shared/ui/toast/toaster.tsx
// Renders the toast deck directly — no Sonner or similar library. Sonner's own
// positioning/stacking/swipe-dismiss fought this app's card-deck + swipe-to-rotate
// toast feature, so this owns layout end to end instead.

import { useRef, useState } from "react";
import { PanResponder, StyleSheet, View } from "react-native";

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
/** How far the front card follows the finger before a swipe turns it. */
const FOLLOW_RATIO = 0.35;
const FOLLOW_MAX_PX = 28;

/**
 * The deck: every toast at one point, the front card on top and the next
 * two peeking out under it. Swiping the front card up or down turns the deck.
 */
export function Toaster() {
  const entries = useToastStore((s) => s.entries);
  const rotation = useToastStore((s) => s.rotation);
  const [follow, setFollow] = useState(0);

  const swipe = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!swipe.current) {
    // The deck turns the moment a swipe crosses the line, not on release,
    // and once a swipe: holding past it does not keep turning.
    let turned = false;
    // Every report of the finger is read the same way, the one that claimed
    // the gesture and the one it lifted on included: a flick can cross the
    // line inside either, with no move reported between.
    const follow = (dy: number) => {
      if (turned) return;
      if (Math.abs(dy) > SWIPE_PX) {
        turned = true;
        setFollow(0);
        toastRotate(dy < 0 ? 1 : -1);
        return;
      }
      setFollow(Math.max(-FOLLOW_MAX_PX, Math.min(FOLLOW_MAX_PX, dy * FOLLOW_RATIO)));
    };
    const end = (dy: number) => {
      follow(dy);
      setFollow(0);
      setToastDragging(false);
    };
    swipe.current = PanResponder.create({
      // Claimed on the press, not on the first move: a pointer's move is
      // offered to whatever it is over by then, which a swipe down has left
      // for the mode bar. A card has no tap of its own to give up for it.
      onStartShouldSetPanResponder: () => useToastStore.getState().entries.length > 1,
      onPanResponderGrant: (_e, g) => {
        turned = false;
        setToastDragging(true);
        follow(g.dy);
      },
      onPanResponderMove: (_e, g) => follow(g.dy),
      onPanResponderRelease: (_e, g) => end(g.dy),
      onPanResponderTerminate: (_e, g) => end(g.dy),
    });
  }

  return (
    <View style={styles.anchor}>
      {entries.map((entry, i) => {
        const depth = toastDepth(i, rotation, entries.length);
        if (depth > MAX_STACK_DEPTH) return null;
        const front = depth === 0;
        return (
          <View
            key={entry.id}
            {...(front ? swipe.current!.panHandlers : {})}
            hitSlop={8}
            style={[
              styles.card,
              {
                zIndex: MAX_STACK_DEPTH - depth,
                pointerEvents: front ? "auto" : "none",
                opacity: STACK_OPACITY[depth],
                transform: [{ translateY: depth * STACK_PEEK_PX + (front ? follow : 0) }, { scale: STACK_SCALE[depth] }],
              },
            ]}
          >
            <Toast message={entry.message} tone={entry.tone} />
          </View>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  // Every card stands on this one point; no card is placed by measuring
  // another, which is what broke repeatedly under Sonner.
  anchor: { alignItems: "center", height: 32, pointerEvents: "box-none" },
  card: { position: "absolute", bottom: 0, transformOrigin: "center bottom" },
});
