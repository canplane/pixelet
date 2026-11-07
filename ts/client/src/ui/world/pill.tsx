// @pxl/client/ui/world/pill.tsx

/**
 * A pill whose place and width move. On a page and on Android it moves by
 * transforms alone: scaling squashes its round ends, and animating width
 * ghosts over the canvas on iOS WebKit. So a fixed circle at its left end and
 * a full-width body sliding out under a clip from the circle's center. The
 * circle lies wholly under the body, since two pieces cut to meet left a
 * hairline; shadows are clipped apart to avoid doubling, which meets cleanly
 * only on whole pixels. An iPhone relays it out and the system animates
 * (pill.ios.tsx).
 */

import type { ReactNode } from "react";
import {
  type ColorValue,
  type GestureResponderHandlers,
  StyleSheet,
  View,
  type ViewStyle,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { BackingPlace } from "@/ui/component/backing.tsx";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { SNAP, type Transition } from "@/ui/component/motion/transition.ts";
import { useDim } from "@/ui/component/motion/dim.ts";
import { touchOutset } from "@/ui/component/touch.ts";
import { pillStyles } from "./pill-style.ts";

export interface PillProps {
  x: f64;
  width: f64;
  height: f64;
  span: f64;
  fill: ColorValue;
  tint?: ColorValue;
  place: BackingPlace;
  interactive?: boolean;
  border?: { width: f64; color: ColorValue };
  shadow?: string;
  fade?: f64;
  fadeFrom?: f64;
  fadeTransition?: Transition;
  touch?: GestureResponderHandlers;
  /** More of `touch`'s outset under it, over a deck's cards peeking out. */
  touchBelow?: f64;
  children?: ReactNode;
  onContentWidth?: (width: f64) => void;
}

const SHADOW_ROOM = 32;
// `tint`, `place` and `interactive` only apply on an iPhone.
export function Pill({
  x,
  width,
  height,
  span,
  fill,
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
  const transition = SNAP;
  const dim = useDim(fade, fadeTransition, fadeFrom);
  const r = height / 2;
  const slide = width - span;
  const edge: ViewStyle = {
    borderRadius: r,
    ...(border && { borderWidth: border.width, borderColor: border.color }),
  };
  const clipLeft = {
    left: -SHADOW_ROOM,
    top: -SHADOW_ROOM,
    width: SHADOW_ROOM + r,
    height: height + 2 * SHADOW_ROOM,
  };
  const clipRight = {
    left: r,
    top: -SHADOW_ROOM,
    width: span - r + SHADOW_ROOM,
    height: height + 2 * SHADOW_ROOM,
  };
  const body = {
    position: "absolute" as const,
    left: -r,
    top: SHADOW_ROOM,
    width: span,
    height,
  };
  const outset = touch ? touchOutset(height) : 0;

  return (
    <Motion to={{ translateX: x }} transition={transition} style={styles.at}>
      <Motion
        {...dim.motion}
        style={[
          styles.fade,
          {
            width: span + 2 * SHADOW_ROOM,
            height: height + 2 * SHADOW_ROOM,
          },
        ]}
      >
        {
          // Never flattened: as `touch` came and went, Android's Fabric lost the
          // unflattened box's create and died adding the pill into it.
        }
        <View collapsable={false} style={styles.corner}>
          <View
            collapsable={false}
            {...touch}
            style={[
              touch ? pillStyles.target : pillStyles.box,
              {
                left: -outset,
                top: -outset,
                width: width + 2 * outset,
                height: height + 2 * outset + touchBelow,
              },
            ]}
          >
            <View
              collapsable={false}
              style={[
                pillStyles.box,
                { left: outset, top: outset, width, height },
              ]}
            >
              {shadow && (
                <>
                  <View style={[styles.clip, clipLeft]}>
                    <View
                      style={{
                        position: "absolute",
                        left: SHADOW_ROOM,
                        top: SHADOW_ROOM,
                        width: height,
                        height,
                        borderRadius: r,
                        boxShadow: shadow,
                      }}
                    />
                  </View>
                  <View style={[styles.clip, clipRight]}>
                    <Motion
                      to={{ translateX: slide }}
                      transition={transition}
                      style={body}
                    >
                      <View
                        style={[
                          styles.fill,
                          {
                            borderRadius: r,
                            boxShadow: shadow,
                          },
                        ]}
                      />
                    </Motion>
                  </View>
                </>
              )}

              <View
                style={[
                  edge,
                  styles.circle,
                  {
                    width: height,
                    height,
                    backgroundColor: fill,
                  },
                ]}
              />
              <View style={[styles.clip, clipRight]}>
                <Motion
                  to={{ translateX: slide }}
                  transition={transition}
                  style={body}
                >
                  <View
                    style={[edge, styles.fill, { backgroundColor: fill }]}
                  />
                </Motion>
              </View>

              <Motion
                to={{ translateX: slide }}
                transition={transition}
                style={[styles.window, { width: span, height }]}
              >
                <Motion
                  to={{ translateX: -slide }}
                  transition={transition}
                  onLayout={
                    onContentWidth &&
                    ((e) => onContentWidth(e.nativeEvent.layout.width))
                  }
                  style={[pillStyles.content, { height }]}
                >
                  {children}
                </Motion>
              </Motion>
            </View>
          </View>
        </View>
      </Motion>
    </Motion>
  );
}

const styles = StyleSheet.create({
  at: { position: "absolute", left: 0, top: 0, pointerEvents: "box-none" },
  fade: {
    position: "absolute",
    left: -SHADOW_ROOM,
    top: -SHADOW_ROOM,
    pointerEvents: "box-none",
  },
  corner: {
    position: "absolute",
    left: SHADOW_ROOM,
    top: SHADOW_ROOM,
    pointerEvents: "box-none",
  },
  clip: { position: "absolute", overflow: "hidden", pointerEvents: "none" },
  circle: { position: "absolute", left: 0, top: 0, pointerEvents: "none" },
  fill: { flex: 1 },
  window: {
    position: "absolute",
    left: 0,
    top: 0,
    overflow: "hidden",
    pointerEvents: "box-none",
  },
});
