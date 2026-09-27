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
import { PanResponder, Pressable, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { useWindowSize } from "@platform/viewport.ts";
import { ease, SLIDE, SLIDE_AWAY } from "@/shared/ui/motion/transition.ts";
import { PANEL_HEAD_HEIGHT, PanelPresence, type PanelProps, SHEET_RADIUS } from "@/shared/ui/panel.tsx";
import { alpha, chrome, color } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

const MAX_WIDTH = 540;
/** Gap between the safe-area top and a full-height sheet. */
const WHOLE_GAP = 12;
/** Release speed, in points per millisecond, that counts as a fling. */
const FLING = 0.5;
/** Follows the finger with no delay. */
const FOLLOW = ease(0, [0, 0, 1, 1]);
/** The handle's bar. */
const HANDLE = { width: 36, height: 4 };
/** How far a press may wander and still be a tap rather than a drag. */
const TAP_SLOP = 8;

export function Panel({ label, onClose, half, children }: PanelProps) {
  const window = useWindowSize();
  const { top } = useSafeArea();
  const { open, gone } = useContext(PanelPresence);
  const width = Math.min(window.width, MAX_WIDTH);
  const fullHeight = window.height - top - WHOLE_GAP;
  const halfHeight = window.height / 2;

  const [full, setFull] = useState(!half);
  // The content area is laid out full height from the moment the sheet
  // grows, and back at half once it has slid down (header comment).
  const [bodyFull, setBodyFull] = useState(full);
  const [drag, setDrag] = useState<f64 | null>(null);
  const shown = full ? fullHeight : halfHeight;

  const detent = (toFull: boolean) => {
    setFull(toFull);
    if (toFull) setBodyFull(true);
  };
  /** Where a release can settle, lowest first: closed, half when the panel
   * has one, full. */
  const stops = [
    { height: 0, settle: onClose },
    ...(half ? [{ height: halfHeight, settle: () => detent(false) }] : []),
    { height: fullHeight, settle: () => detent(true) },
  ];

  // What the responder, made once, reads of the latest render.
  const live = useRef({ top: window.height - shown, center: window.width / 2, shown, full, half, stops });
  live.current = { top: window.height - shown, center: window.width / 2, shown, full, half, stops };

  const responder = useRef(
    PanResponder.create({
      // Claimed at the press, not at the first move: a move is offered to the
      // element under the pointer, which is the map once a drag upward leaves
      // the sheet. Bubbling, so the close button, which is deeper, still
      // takes its taps. The handle is no button of its own: one would take
      // every press in its square, drags as well.
      onStartShouldSetPanResponder: (e) => e.nativeEvent.pageY - live.current.top < PANEL_HEAD_HEIGHT,
      onPanResponderTerminationRequest: () => false,
      onPanResponderMove: (_, g) => setDrag(g.dy),
      onPanResponderRelease: (_, g) => {
        setDrag(null);
        const { top, center, shown, full, half, stops } = live.current;
        const tap = Math.abs(g.dx) < TAP_SLOP && Math.abs(g.dy) < TAP_SLOP;
        if (tap) {
          // A tap on the handle's square toggles half and full.
          const onHandle = g.y0 - top < chrome.target && Math.abs(g.x0 - center) < chrome.target / 2;
          if (half && onHandle) detent(!full);
          return;
        }
        const height = shown - g.dy;
        const stop = g.vy < -FLING
          ? stops.find((s) => s.height > shown)
          : g.vy > FLING
          ? stops.findLast((s) => s.height < shown)
          : stops.reduce((a, b) => (Math.abs(b.height - height) < Math.abs(a.height - height) ? b : a));
        stop?.settle();
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
        onSettled={open ? () => drag === null && setBodyFull(full) : gone}
        style={[styles.sheet, { width, height: fullHeight, left: (window.width - width) / 2 }]}
      >
        <View {...responder.panHandlers} accessibilityViewIsModal accessibilityLabel={label} style={[styles.body, { height: bodyFull ? fullHeight : halfHeight }]}>
          {children}
          {half && <View style={styles.handle} />}
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
    backgroundColor: color.ground,
    boxShadow: "0 -4px 20px rgba(0, 0, 0, 0.12)",
  },
  // The sheet's handle, 4 below the top edge. A tap lands on the
  // least target's square around it (the responder's release).
  handle: { position: "absolute", top: 4, left: "50%", marginLeft: -HANDLE.width / 2, ...HANDLE, borderRadius: HANDLE.height / 2, pointerEvents: "none", backgroundColor: LABEL.tertiary },
});
