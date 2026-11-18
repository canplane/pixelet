// @pxl/client/ui/feature/world/pill.ios.tsx

/**
 * An iPhone's pill is the web's where it is solid or the system has no glass.
 * Of glass, it is one piece, laid out again at each new place and width and
 * animated by LayoutAnimation, because clipped pieces would show seams through
 * the glass. Its content fades on the glass, never over it.
 */

import { useRef } from "react";
import {
  LayoutAnimation,
  type LayoutAnimationConfig,
  StyleSheet,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Backing, SYSTEM_BACKING } from "@/ui/shared/component/backing.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { PILL_STYLE } from "./pill-style.ts";
import { type PillProps, Pill as Pieces } from "./pill.tsx";
import { touchOutset } from "@/ui/shared/component/touch.ts";

// This file exports everything pill.tsx does, with its own `Pill` in place of
// that one.
export * from "./pill.tsx";

/** Only size and place animate. A view that comes or goes does so at once. */
const MOVE: LayoutAnimationConfig = {
  duration: SNAP.durationMs,
  update: { type: "spring", springDamping: 1 },
};

export function Pill(props: PillProps) {
  return props.solid || !SYSTEM_BACKING ? (
    <Pieces {...props} />
  ) : (
    <Glass {...props} />
  );
}

function Glass({
  x,
  width,
  height,
  fill,
  tint,
  place,
  interactive,
  fade = 1,
  fadeFrom,
  fadeTransition = SNAP,
  touch,
  touchBelow = 0,
  children,
  onContentWidth,
}: PillProps) {
  // This runs during render, because a layout animation applies to the next
  // commit.
  const last = useRef<[f64, f64] | null>(null);
  if (
    last.current !== null &&
    (last.current[0] !== x || last.current[1] !== width)
  )
    LayoutAnimation.configureNext(MOVE);
  last.current = [x, width];

  const r = height / 2;
  const outset = touch ? touchOutset(height) : 0;
  // The content stands inside the glass, because interactive glass answers
  // only the touches that reach it through its own content.
  return (
    <View
      style={[
        touch ? PILL_STYLE.target : PILL_STYLE.box,
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
        style={[STYLE.glass, { borderRadius: r }]}
      >
        <View
          style={[StyleSheet.absoluteFill, STYLE.clip, { borderRadius: r }]}
        >
          <View
            onLayout={
              onContentWidth &&
              ((e) => onContentWidth(e.nativeEvent.layout.width))
            }
            style={[PILL_STYLE.content, { height }]}
          >
            {children}
          </View>
        </View>
      </Backing>
    </View>
  );
}

const STYLE = StyleSheet.create({
  glass: { flex: 1 },
  clip: { overflow: "hidden", pointerEvents: "box-none" },
});
