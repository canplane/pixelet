// @pxl/client/ui/feature/world/pill.tsx

/**
 * A pill whose place and width can move. On a page and on Android it moves by
 * transforms alone. It does not scale, because scaling squashes its round
 * ends, and it does not animate its width, because that leaves ghosts over
 * the canvas in iOS WebKit. So it is drawn as a fixed circle at its left end
 * and a full-width body that slides out from under a clip at the circle's
 * center. The circle lies wholly under the body, because two pieces cut to
 * meet leave a hairline between them. The two pieces' shadows are clipped
 * apart so that they do not double, and they meet cleanly only on whole
 * pixels. An iPhone lays the pill out again and lets the system animate it
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
import type { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import {
  SNAP,
  type Transition,
} from "@/ui/shared/component/motion/transition.ts";
import { useDim } from "@/ui/shared/component/motion/dim.ts";
import { touchOutset } from "@/ui/shared/component/touch.ts";
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
  /** How much further `touch`'s area reaches below the pill, over the cards of
   * a deck that peek out under it. */
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
          // These views are never flattened. When `touch` comes and goes, the
          // box turns from flattened to not, and Android's Fabric renderer can
          // lose the box's creation and crash adding the pill into it.
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
