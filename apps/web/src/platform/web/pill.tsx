// @pxl/web/platform/web/pill.tsx

/**
 * A pill on the page (shared/ui/pill.ts): its place and its width move
 * without anything being laid out, by transforms alone
 * (shared/ui/motion/pose.ts).
 *
 * Scaling a pill would squash its round ends into ovals, and animating its
 * width reflows and ghosts over the canvas on iOS. So it is two pieces that
 * never change size. A circle stands still at its left end; its body, a pill
 * as wide as the widest it will be asked to show, slides out from behind it
 * under a clip that starts at the circle's center. What shows is the circle's
 * left half and the body's right part, which is a pill of exactly the width
 * asked for, round at both ends at every moment of the slide.
 *
 * The whole circle lies under the body, so the clip's edge always falls on
 * the same fill and has nothing to show through: two pieces cut to meet on
 * one line left a hairline there, which a compositor snapping the two clips
 * apart by a device pixel opened on whole pixels as well. Shadows cannot
 * overlap that way — two would darken each other — so they are cast apart,
 * each piece's inside a clip of its own either side of the center, under
 * both fills. They meet cleanly only on a pixel's edge, so a caller stands
 * the pill on whole pixels (PixelRatio.roundToNearestPixel).
 *
 * What it holds is revealed the same way: laid out once at its full width,
 * in a window that slides with the body while the content slides back by as
 * much, so the content stands still and its right edge follows the pill's.
 */

import { StyleSheet, View, type ViewStyle } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import type { PillProps } from "@/shared/ui/pill.ts";

/** Room for a shadow around a piece, inside the clip that holds it. */
const SHADOW_ROOM = 32;
/** How far past its edge a touch still lands on it. */
const TOUCH_SLOP = 8;

// The web has no system glass (glass.tsx), so `tint`, `variant` and
// `interactive` only apply on a device.
export function Pill({ x, width, height, span, fill, border, shadow, fade = 1, fadeFrom, fadeTransition = SNAP, touch, children, onContentWidth }: PillProps) {
  const transition = SNAP;
  const veil = useVeil(fade, fadeTransition);
  const r = height / 2;
  const slide = width - span;
  const edge: ViewStyle = { borderRadius: r, ...(border && { borderWidth: border.width, borderColor: border.color }) };
  const clipLeft = { left: -SHADOW_ROOM, top: -SHADOW_ROOM, width: SHADOW_ROOM + r, height: height + 2 * SHADOW_ROOM };
  const clipRight = { left: r, top: -SHADOW_ROOM, width: span - r + SHADOW_ROOM, height: height + 2 * SHADOW_ROOM };
  const body = { position: "absolute" as const, left: -r, top: SHADOW_ROOM, width: span, height };

  return (
    <Motion to={{ translateX: x }} transition={transition} style={styles.at}>
      <Motion from={fadeFrom === undefined ? undefined : { opacity: fadeFrom }} to={{ opacity: veil.level }} transition={veil.transition} style={styles.fade}>
        <View {...touch} hitSlop={touch && TOUCH_SLOP} style={[styles.box, { width, height }]}>
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
      </Motion>
    </Motion>
  );
}

const styles = StyleSheet.create({
  at: { position: "absolute", left: 0, top: 0, pointerEvents: "box-none" },
  fade: { pointerEvents: "box-none" },
  // From the sheet: an inline `box-none` is drawn as a `none` the content
  // inherits (feature/map/ui/busy-dim.tsx).
  box: { pointerEvents: "box-none" },
  clip: { position: "absolute", overflow: "hidden", pointerEvents: "none" },
  circle: { position: "absolute", left: 0, top: 0, pointerEvents: "none" },
  fill: { flex: 1 },
  window: { position: "absolute", left: 0, top: 0, overflow: "hidden", pointerEvents: "box-none" },
  content: { position: "absolute", left: 0, top: 0, flexDirection: "row", alignItems: "center", pointerEvents: "box-none" },
});
