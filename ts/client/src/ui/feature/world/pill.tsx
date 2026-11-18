// @pxl/client/ui/feature/world/pill.tsx

/**
 * A pill whose place and width can move. On the web and on Android it moves by
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
import { PILL_STYLE } from "./pill-style.ts";

export interface PillProps {
  x: f64;
  width: f64;
  height: f64;
  spanPx: f64;
  fill: ColorValue;
  tint?: ColorValue;
  solid?: boolean;
  place: BackingPlace;
  interactive?: boolean;
  border?: { width: f64; color: ColorValue };
  shadow?: string;
  fade?: f64;
  fadeFrom?: f64;
  fadeTransition?: Transition;
  /** Whether the pill takes the touches on it, so that none reaches what lies
   * under it. */
  touch?: boolean;
  /** How much further `touch`'s area reaches below the pill, over the cards of
   * a deck that peek out under it. */
  touchBelow?: f64;
  children?: ReactNode;
  onContentWidth?: (width: f64) => void;
}

const SHADOW_ROOM = 32;
// `tint`, `solid`, `place` and `interactive` only apply on an iPhone.
export function Pill({
  x,
  width,
  height,
  spanPx,
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
  const slide = width - spanPx;
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
    width: spanPx - r + SHADOW_ROOM,
    height: height + 2 * SHADOW_ROOM,
  };
  const body = {
    position: "absolute" as const,
    left: -r,
    top: SHADOW_ROOM,
    width: spanPx,
    height,
  };
  const outset = touch ? touchOutset(height) : 0;

  return (
    <Motion to={{ translateX: x }} transition={transition} style={STYLE.at}>
      <Motion
        {...dim.motion}
        style={[
          STYLE.fade,
          {
            width: spanPx + 2 * SHADOW_ROOM,
            height: height + 2 * SHADOW_ROOM,
          },
        ]}
      >
        {
          // These views are never flattened. When `touch` comes and goes, the
          // box turns from flattened to not, and Android's Fabric renderer can
          // lose the box's creation and crash adding the pill into it.
        }
        <View collapsable={false} style={STYLE.corner}>
          <View
            collapsable={false}
            style={[
              touch ? PILL_STYLE.target : PILL_STYLE.box,
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
                PILL_STYLE.box,
                { left: outset, top: outset, width, height },
              ]}
            >
              {shadow && (
                <>
                  <View style={[STYLE.clip, clipLeft]}>
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
                  <View style={[STYLE.clip, clipRight]}>
                    <Motion
                      to={{ translateX: slide }}
                      transition={transition}
                      style={body}
                    >
                      <View
                        style={[
                          STYLE.fill,
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
                  STYLE.circle,
                  {
                    width: height,
                    height,
                    backgroundColor: fill,
                  },
                ]}
              />
              <View style={[STYLE.clip, clipRight]}>
                <Motion
                  to={{ translateX: slide }}
                  transition={transition}
                  style={body}
                >
                  <View style={[edge, STYLE.fill, { backgroundColor: fill }]} />
                </Motion>
              </View>

              <Motion
                to={{ translateX: slide }}
                transition={transition}
                style={[STYLE.window, { width: spanPx, height }]}
              >
                <Motion
                  to={{ translateX: -slide }}
                  transition={transition}
                  onLayout={
                    onContentWidth &&
                    ((e) => onContentWidth(e.nativeEvent.layout.width))
                  }
                  style={[PILL_STYLE.content, { height }]}
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

const STYLE = StyleSheet.create({
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
