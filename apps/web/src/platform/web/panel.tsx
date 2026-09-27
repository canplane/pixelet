// @pxl/web/platform/web/panel.tsx

/**
 * A panel on the web: a bottom sheet like the device's (the device app's
 * platform/panel.tsx), over a dimmed map; tapping the map closes it. It
 * opens at full height, or half when `half`. As on the device, it is dragged
 * by its header area only: between half and full, and down to close. A tap
 * on the handle toggles half and full, for a mouse.
 *
 * The sheet is always laid out at full height and shown at half by sliding
 * it down, so the drag and the snap animate transform only; the content
 * area's height follows the detent (at once when growing, after the slide
 * when shrinking) so its scroll reaches its end at either height.
 *
 * Max width is 540, the width of an iPad form sheet, so rows do not stretch
 * across a desktop screen.
 */

import { useContext, useRef, useState } from "react";
import { PanResponder, Pressable, StyleSheet, useWindowDimensions, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { ease, SLIDE, SLIDE_AWAY } from "@/shared/ui/motion/transition.ts";
import { PANEL_HEAD_HEIGHT, PanelPresence, type PanelProps, SHEET_RADIUS } from "@/shared/ui/panel.tsx";
import { alpha, color } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

const MAX_WIDTH = 540;
/** Gap between the safe-area top and a full-height sheet, as on iOS. */
const WHOLE_GAP = 10;
/** Release speed, in points per millisecond, that counts as a fling. */
const FLING = 0.5;
/** Follows the finger with no delay. */
const FOLLOW = ease(0, [0, 0, 1, 1]);

export function Panel({ label, onClose, half, children }: PanelProps) {
  const window = useWindowDimensions();
  const { top } = useSafeArea();
  const { open, gone } = useContext(PanelPresence);
  const width = Math.min(window.width, MAX_WIDTH);
  const fullHeight = window.height - top - WHOLE_GAP;
  const halfHeight = window.height / 2;

  const [full, setFull] = useState(!half);
  const [drag, setDrag] = useState<f64 | null>(null);
  const shown = full ? fullHeight : halfHeight;
  const [bodyHeight, setBodyHeight] = useState(shown);

  const toFull = () => {
    setFull(true);
    setBodyHeight(fullHeight);
  };
  const toHalf = () => setFull(false);

  // Read by the responder, which is made once.
  const now = useRef({ shown, half, fullHeight, halfHeight, windowHeight: window.height });
  now.current = { shown, half, fullHeight, halfHeight, windowHeight: window.height };
  const act = useRef({ toFull, toHalf, onClose });
  act.current = { toFull, toHalf, onClose };

  const responder = useRef(
    PanResponder.create({
      // Claimed at the press, not at the first move: a move is offered to the
      // element under the pointer, which is the map once a drag upward leaves
      // the sheet. Bubbling, so the close button and the handle, which are
      // deeper, still take their taps.
      onStartShouldSetPanResponder: (e) => {
        const { shown, windowHeight } = now.current;
        return e.nativeEvent.pageY - (windowHeight - shown) < PANEL_HEAD_HEIGHT;
      },
      onPanResponderTerminationRequest: () => false,
      onPanResponderMove: (_, g) => setDrag(g.dy),
      onPanResponderRelease: (_, g) => {
        setDrag(null);
        const { shown, half, fullHeight, halfHeight } = now.current;
        const height = shown - g.dy;
        const stops = half ? [0, halfHeight, fullHeight] : [0, fullHeight];
        let stop: f64;
        if (g.vy < -FLING) stop = stops.find((s) => s > shown) ?? shown;
        else if (g.vy > FLING) stop = [...stops].reverse().find((s) => s < shown) ?? 0;
        else stop = stops.reduce((a, b) => (Math.abs(b - height) < Math.abs(a - height) ? b : a));
        if (stop === 0) act.current.onClose();
        else if (stop === fullHeight) act.current.toFull();
        else act.current.toHalf();
      },
      onPanResponderTerminate: () => setDrag(null),
    }),
  ).current;

  // At rest the sheet stands `shown` tall above the bottom; a drag moves it
  // with the finger, never above full height.
  const rest = fullHeight - shown;
  const offset = drag === null ? rest : Math.max(0, rest + drag);
  // Past its own edge and its shadow, below the screen.
  const away = { translateY: fullHeight + 16 };

  return (
    <View style={[styles.layer, open ? styles.pass : styles.block]}>
      <Motion from={{ opacity: 0 }} to={{ opacity: open ? 1 : 0 }} transition={open ? SLIDE : SLIDE_AWAY} style={StyleSheet.absoluteFill}>
        <Pressable accessible={false} onPress={onClose} style={styles.dim} />
      </Motion>
      <Motion
        from={away}
        to={open ? { translateY: offset } : away}
        transition={drag !== null ? FOLLOW : open ? SLIDE : SLIDE_AWAY}
        onSettled={open ? () => drag === null && !full && setBodyHeight(halfHeight) : gone}
        style={[styles.sheet, { width, height: fullHeight, left: (window.width - width) / 2 }]}
      >
        <View {...responder.panHandlers} accessibilityViewIsModal accessibilityLabel={label} style={[styles.body, { height: bodyHeight }]}>
          {children}
          {half && (
            <Pressable
              onPress={full ? toHalf : toFull}
              accessibilityRole="adjustable"
              accessibilityLabel={label}
              hitSlop={8}
              style={styles.handleArea}
            >
              <View style={styles.handle} />
            </Pressable>
          )}
        </View>
      </Motion>
    </View>
  );
}

const styles = StyleSheet.create({
  // Over the map and its toasts.
  layer: { position: "absolute", top: 0, right: 0, bottom: 0, left: 0, zIndex: 70 },
  body: { width: "100%" },
  // From the sheet, not inline, where react-native-web would draw `box-none`
  // as a `none` the panel inherits (feature/map/ui/busy-dim.tsx).
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
  dim: { flex: 1, backgroundColor: alpha(color.black, 0.2) },
  sheet: {
    position: "absolute",
    bottom: 0,
    overflow: "hidden",
    borderTopLeftRadius: SHEET_RADIUS,
    borderTopRightRadius: SHEET_RADIUS,
    backgroundColor: color.paper,
    boxShadow: "0 -4px 20px rgba(0, 0, 0, 0.12)",
  },
  // iOS's sheet handle: 36 × 5, 5 below the top edge.
  handleArea: { position: "absolute", top: 0, left: "50%", marginLeft: -24, width: 48, height: 15, alignItems: "center", paddingTop: 5 },
  handle: { width: 36, height: 5, borderRadius: 2.5, backgroundColor: LABEL.tertiary },
});
