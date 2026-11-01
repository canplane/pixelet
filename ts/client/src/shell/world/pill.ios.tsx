// @pxl/client/shell/world/pill.ios.tsx

/**
 * A pill on a device: one piece of glass relaid at its new place and width,
 * animated by LayoutAnimation, since clipped pieces showed seams through the
 * glass. Content fades on the glass, never over it.
 */

import { useRef } from "react";
import {
  LayoutAnimation,
  type LayoutAnimationConfig,
  StyleSheet,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Backing } from "@/shell/ui/backing.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import type { PillProps } from "./pill.tsx";
import { touchOutset } from "@/shell/ui/touch.ts";

// Everything the default has, but what this file draws itself.
export * from "./pill.tsx";

/** Only size and place animate; a view that comes or goes does so at once. */
const MOVE: LayoutAnimationConfig = {
  duration: SNAP.durationMs,
  update: { type: "spring", springDamping: 1 },
};

export function Pill({
  x,
  width,
  height,
  fill,
  tint,
  place,
  interactive,
  border,
  shadow,
  fade = 1,
  fadeFrom,
  fadeTransition = SNAP,
  touch,
  touchBelow = 0,
  children,
  onContentWidth,
}: PillProps) {
  // In render: a layout animation applies to the next commit.
  const last = useRef<[f64, f64] | null>(null);
  if (
    last.current !== null &&
    (last.current[0] !== x || last.current[1] !== width)
  )
    LayoutAnimation.configureNext(MOVE);
  last.current = [x, width];

  const r = height / 2;
  const outset = touch ? touchOutset(height) : 0;
  // Content stands inside the glass: interactive glass answers only touches
  // given through what it contains.
  return (
    <View
      {...touch}
      style={[
        touch ? styles.target : styles.at,
        {
          left: x - outset,
          top: -outset,
          width: width + 2 * outset,
          height: height + 2 * outset + touchBelow,
          padding: outset,
          paddingBottom: outset + touchBelow,
        },
      ]}
    >
      <Backing
        fill={fill}
        tint={tint}
        place={place}
        interactive={interactive}
        fade={fade}
        fadeFrom={fadeFrom}
        transition={fadeTransition}
        style={[
          styles.glass,
          { borderRadius: r },
          border && { borderWidth: border.width, borderColor: border.color },
          shadow ? { boxShadow: shadow } : null,
        ]}
      >
        <View
          style={[StyleSheet.absoluteFill, styles.clip, { borderRadius: r }]}
        >
          <View
            onLayout={
              onContentWidth &&
              ((e) => onContentWidth(e.nativeEvent.layout.width))
            }
            style={[styles.content, { height }]}
          >
            {children}
          </View>
        </View>
      </Backing>
    </View>
  );
}

const styles = StyleSheet.create({
  at: { position: "absolute", pointerEvents: "box-none" },
  target: { position: "absolute" },
  glass: { flex: 1 },
  clip: { overflow: "hidden", pointerEvents: "box-none" },
  content: {
    position: "absolute",
    left: 0,
    top: 0,
    flexDirection: "row",
    alignItems: "center",
    pointerEvents: "box-none",
  },
});
