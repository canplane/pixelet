// @pxl/web/platform/web/pill.tsx

/**
 * A pill on the page, moved by transforms alone: scaling squashes its round
 * ends, and animating width ghosts over the canvas on iOS. So a fixed circle
 * at its left end and a full-width body sliding out under a clip from the
 * circle's center. The circle lies wholly under the body, since two pieces
 * cut to meet left a hairline; shadows are clipped apart to avoid doubling,
 * which meets cleanly only on whole pixels.
 */

import { StyleSheet, View, type ViewStyle } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { type PillProps, touchReach } from "@/shared/ui/pill.ts";

const SHADOW_ROOM = 32;
// `tint`, `place` and `interactive` only apply on an iPhone.
export function Pill({ x, width, height, span, fill, border, shadow, fade = 1, fadeFrom, fadeTransition = SNAP, touch, touchBelow = 0, children, onContentWidth }: PillProps) {
  const transition = SNAP;
  const veil = useVeil(fade, fadeTransition, fadeFrom);
  const r = height / 2;
  const slide = width - span;
  const edge: ViewStyle = { borderRadius: r, ...(border && { borderWidth: border.width, borderColor: border.color }) };
  const clipLeft = { left: -SHADOW_ROOM, top: -SHADOW_ROOM, width: SHADOW_ROOM + r, height: height + 2 * SHADOW_ROOM };
  const clipRight = { left: r, top: -SHADOW_ROOM, width: span - r + SHADOW_ROOM, height: height + 2 * SHADOW_ROOM };
  const body = { position: "absolute" as const, left: -r, top: SHADOW_ROOM, width: span, height };
  const reach = touch ? touchReach(height) : 0;

  return (
    <Motion to={{ translateX: x }} transition={transition} style={styles.at}>
      <Motion
        {...veil.motion}
        style={[styles.fade, { width: span + 2 * SHADOW_ROOM, height: height + 2 * SHADOW_ROOM }]}
      >
        <View style={styles.corner}>
          <View {...touch} style={[touch ? styles.reach : styles.box, { left: -reach, top: -reach, width: width + 2 * reach, height: height + 2 * reach + touchBelow }]}>
            <View style={[styles.box, { left: reach, top: reach, width, height }]}>
              {shadow && (
                <>
                  <View style={[styles.clip, clipLeft]}>
                    <View style={{ position: "absolute", left: SHADOW_ROOM, top: SHADOW_ROOM, width: height, height, borderRadius: r, boxShadow: shadow }} />
                  </View>
                  <View style={[styles.clip, clipRight]}>
                    <Motion to={{ translateX: slide }} transition={transition} style={body}>
                      <View style={[styles.fill, { borderRadius: r, boxShadow: shadow }]} />
                    </Motion>
                  </View>
                </>
              )}

              <View style={[edge, styles.circle, { width: height, height, backgroundColor: fill }]} />
              <View style={[styles.clip, clipRight]}>
                <Motion to={{ translateX: slide }} transition={transition} style={body}>
                  <View style={[edge, styles.fill, { backgroundColor: fill }]} />
                </Motion>
              </View>

              <Motion to={{ translateX: slide }} transition={transition} style={[styles.window, { width: span, height }]}>
                <Motion
                  to={{ translateX: -slide }}
                  transition={transition}
                  onLayout={onContentWidth && ((e) => onContentWidth(e.nativeEvent.layout.width))}
                  style={[styles.content, { height }]}
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
  fade: { position: "absolute", left: -SHADOW_ROOM, top: -SHADOW_ROOM, pointerEvents: "box-none" },
  corner: { position: "absolute", left: SHADOW_ROOM, top: SHADOW_ROOM, pointerEvents: "box-none" },
  // From the sheet: an inline `box-none` is drawn as a `none` the content inherits.
  box: { position: "absolute", pointerEvents: "box-none" },
  reach: { position: "absolute" },
  clip: { position: "absolute", overflow: "hidden", pointerEvents: "none" },
  circle: { position: "absolute", left: 0, top: 0, pointerEvents: "none" },
  fill: { flex: 1 },
  window: { position: "absolute", left: 0, top: 0, overflow: "hidden", pointerEvents: "box-none" },
  content: { position: "absolute", left: 0, top: 0, flexDirection: "row", alignItems: "center", pointerEvents: "box-none" },
});
