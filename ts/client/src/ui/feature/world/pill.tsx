// @pxl/client/ui/feature/world/pill.tsx

/**
 * A pill whose place and width can move, by transforms alone: on the web, on
 * Android, and on an iPhone where it is solid or the system has no glass. It
 * does not scale, because scaling squashes its round ends, and it does not
 * animate its width, because that leaves ghosts over the canvas in iOS WebKit.
 * So it is drawn as a fixed circle at its left end and a full-width body that
 * slides out from under a clip at the circle's center. The circle lies wholly
 * under the body, because two pieces cut to meet leave a hairline between them.
 * The two pieces' shadows are clipped apart so that they do not double, and
 * they meet cleanly only on whole pixels. A shadow's piece is filled too, so
 * that the fill's soft edge lies over the fill and the ground does not show
 * through it. On an iPhone a pill of glass is laid out again and the system
 * animates it (pill.ios.tsx).
 */

import type { ReactNode } from "react";
import { type ColorValue, StyleSheet, View } from "react-native";

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

interface BasePillProps {
  x: f64;
  width: f64;
  height: f64;
  spanPx: f64;
  fill: ColorValue;
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

/** A pill of glass where an iPhone has glass, which `tint` and `place` set;
 * elsewhere it is its fill. */
export interface GlassPillProps extends BasePillProps {
  solid?: false;
  tint?: ColorValue;
  place: BackingPlace;
}

/** A pill drawn as its fill on every platform. */
interface SolidPillProps extends BasePillProps {
  solid: true;
}

export type PillProps = GlassPillProps | SolidPillProps;

const SHADOW_ROOM = 32;
export function Pill({
  x,
  width,
  height,
  spanPx,
  fill,
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
                        backgroundColor: fill,
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
                            backgroundColor: fill,
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
                  STYLE.circle,
                  {
                    borderRadius: r,
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
                  <View
                    style={[
                      STYLE.fill,
                      { borderRadius: r, backgroundColor: fill },
                    ]}
                  />
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
