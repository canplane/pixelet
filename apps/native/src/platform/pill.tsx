// @pxl/native/platform/pill.tsx

/**
 * A pill on a device (shared/ui/pill.ts): one piece of the platform's glass,
 * laid out where it stands and as wide as it shows. A change of either is laid
 * out at once and the system animates the frame on the UI thread
 * (LayoutAnimation), so the glass is redrawn whole at every width — its round
 * ends, its rim and what it bends behind it — where pieces moved under clips
 * showed their seams through it.
 *
 * What it holds stands at its left end, clipped to its round shape, so the
 * content is revealed as the pill opens and hidden as it closes. Its presence
 * is the glass's to show (backing.tsx), and what it holds fades by opacity on
 * the glass, never over it.
 */

import { useRef } from "react";
import { LayoutAnimation, type LayoutAnimationConfig, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Backing } from "@platform/backing.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import type { PillProps } from "@/shared/ui/pill.ts";

/** How far past its edge a touch still lands on it. */
const TOUCH_SLOP = 8;

/** A control's move, as long as the page's (shared/ui/motion/transition.ts).
 * Only what changes size or place is animated: a view that comes or goes in
 * the same commit does so at once, as the page's does. */
const MOVE: LayoutAnimationConfig = {
  duration: SNAP.durationMs,
  update: { type: "spring", springDamping: 1 },
};

export function Pill({ x, width, height, fill, tint, place, interactive, border, shadow, fade = 1, fadeFrom, fadeTransition = SNAP, touch, children, onContentWidth }: PillProps) {
  // Asked for in render, before the commit that lays the new frame out: a
  // layout animation applies to the next commit, and after this one's is too
  // late. The same on a second render of the same props.
  const last = useRef<[f64, f64] | null>(null);
  if (last.current !== null && (last.current[0] !== x || last.current[1] !== width)) LayoutAnimation.configureNext(MOVE);
  last.current = [x, width];

  const r = height / 2;
  // What it holds stands in the glass rather than beside it: a touch reaches
  // the glass only through what the glass contains, and interactive glass
  // answers no touch it is not given.
  return (
    <View {...touch} hitSlop={touch && TOUCH_SLOP} style={[styles.at, { left: x, width, height }]}>
      <Backing
        fill={fill}
        tint={tint}
        place={place}
        interactive={interactive}
        fade={fade}
        fadeFrom={fadeFrom}
        transition={fadeTransition}
        style={[StyleSheet.absoluteFill, { borderRadius: r }, border && { borderWidth: border.width, borderColor: border.color }, shadow ? { boxShadow: shadow } : null]}
      >
        <View style={[StyleSheet.absoluteFill, styles.clip, { borderRadius: r }]}>
          <View onLayout={onContentWidth && ((e) => onContentWidth(e.nativeEvent.layout.width))} style={[styles.content, { height }]}>
            {children}
          </View>
        </View>
      </Backing>
    </View>
  );
}

const styles = StyleSheet.create({
  at: { position: "absolute", top: 0, pointerEvents: "box-none" },
  clip: { overflow: "hidden", pointerEvents: "box-none" },
  content: { position: "absolute", left: 0, top: 0, flexDirection: "row", alignItems: "center", pointerEvents: "box-none" },
});
