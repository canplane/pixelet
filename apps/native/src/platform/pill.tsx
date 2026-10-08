// @pxl/native/platform/pill.tsx

/**
 * A pill on a device: one piece of glass relaid at its new place and width,
 * animated by LayoutAnimation, since clipped pieces showed seams through the
 * glass. Content fades on the glass, never over it.
 */

import { useRef } from "react";
import { LayoutAnimation, type LayoutAnimationConfig, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Backing } from "@platform/backing.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { type PillProps, touchReach } from "@/shared/ui/pill.ts";

/** Only size and place animate; a view that comes or goes does so at once. */
const MOVE: LayoutAnimationConfig = {
  duration: SNAP.durationMs,
  update: { type: "spring", springDamping: 1 },
};

export function Pill({ x, width, height, fill, tint, place, interactive, border, shadow, fade = 1, fadeFrom, fadeTransition = SNAP, touch, touchBelow = 0, children, onContentWidth }: PillProps) {
  // In render: a layout animation applies to the next commit.
  const last = useRef<[f64, f64] | null>(null);
  if (last.current !== null && (last.current[0] !== x || last.current[1] !== width)) LayoutAnimation.configureNext(MOVE);
  last.current = [x, width];

  const r = height / 2;
  const reach = touch ? touchReach(height) : 0;
  // Content stands inside the glass: interactive glass answers only touches
  // given through what it contains.
  return (
    <View
      {...touch}
      style={[touch ? styles.reach : styles.at, { left: x - reach, top: -reach, width: width + 2 * reach, height: height + 2 * reach + touchBelow, padding: reach, paddingBottom: reach + touchBelow }]}
    >
      <Backing
        fill={fill}
        tint={tint}
        place={place}
        interactive={interactive}
        fade={fade}
        fadeFrom={fadeFrom}
        transition={fadeTransition}
        style={[styles.glass, { borderRadius: r }, border && { borderWidth: border.width, borderColor: border.color }, shadow ? { boxShadow: shadow } : null]}
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
  at: { position: "absolute", pointerEvents: "box-none" },
  reach: { position: "absolute" },
  glass: { flex: 1 },
  clip: { overflow: "hidden", pointerEvents: "box-none" },
  content: { position: "absolute", left: 0, top: 0, flexDirection: "row", alignItems: "center", pointerEvents: "box-none" },
});
