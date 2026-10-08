// @pxl/web/platform/web/panel.tsx

/**
 * A bottom sheet over a dimmed map, dragged by its header only. Always laid
 * out full height and slid down to show half, so drags animate transform
 * only. Max width 540, an iPad form sheet's.
 */

import { useContext, useRef, useState } from "react";
import { PanResponder, Pressable, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { useWindowSize } from "@platform/viewport.ts";
import { ease, SLIDE, SLIDE_AWAY } from "@/shared/ui/motion/transition.ts";
import { PANEL_HEAD_HEIGHT, PANEL_TOP, PanelPresence, type PanelProps, SHEET_RADIUS } from "@/shared/ui/panel.tsx";
import { alpha, chrome, color, layer } from "@/shared/ui/theme.ts";
import { touches } from "@/shared/ui/touch.ts";
import { LABEL } from "@platform/palette.ts";

const MAX_WIDTH = 540;
const WHOLE_GAP = 12;
/** Points per millisecond. */
const FLING = 0.5;
const FOLLOW = ease(0, [0, 0, 1, 1]);
/** An iPhone sheet's grabber. */
const HANDLE = { width: 36, height: 5 };
const TAP_SLOP = 8;

export function Panel({ label, onClose, half, children }: PanelProps) {
  const window = useWindowSize();
  const { top } = useSafeArea();
  const { open, gone } = useContext(PanelPresence);
  const width = Math.min(window.width, MAX_WIDTH);
  const fullHeight = window.height - top - WHOLE_GAP;
  const halfHeight = window.height / 2;

  const [full, setFull] = useState(!half);
  // Full height as soon as it grows, back at half once it has slid down, so the scroll reaches its end.
  const [bodyFull, setBodyFull] = useState(full);
  const [drag, setDrag] = useState<f64 | null>(null);
  const shown = full ? fullHeight : halfHeight;

  const detent = (toFull: boolean) => {
    setFull(toFull);
    if (toFull) setBodyFull(true);
  };
  const stops = [
    { height: 0, settle: onClose },
    ...(half ? [{ height: halfHeight, settle: () => detent(false) }] : []),
    { height: fullHeight, settle: () => detent(true) },
  ];

  const now = { top: window.height - shown, center: window.width / 2, shown, full, half, stops };
  const live = useRef(now);
  live.current = now;

  const responder = useRef(
    PanResponder.create({
      // Claimed at the press: a drag up leaves the sheet, and its moves would go
      // to the map. The handle is no button, which would take its drags too.
      onStartShouldSetPanResponder: (e) => e.nativeEvent.pageY - live.current.top < PANEL_HEAD_HEIGHT,
      onPanResponderTerminationRequest: () => false,
      onPanResponderMove: (_, g) => setDrag(g.dy),
      onPanResponderRelease: (_, g) => {
        setDrag(null);
        const { top, center, shown, full, half, stops } = live.current;
        const tap = Math.abs(g.dx) < TAP_SLOP && Math.abs(g.dy) < TAP_SLOP;
        if (tap) {
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

  const rest = fullHeight - shown;
  const offset = drag === null ? rest : Math.max(0, rest + drag);
  const away = { translateY: fullHeight + 16 };

  return (
    <View style={[styles.layer, open ? touches.pass : touches.block]}>
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
          <View style={styles.handle} />
        </View>
      </Motion>
    </View>
  );
}

const styles = StyleSheet.create({
  layer: { ...StyleSheet.absoluteFill, zIndex: layer.sheet },
  body: { width: "100%" },
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
  handle: {
    position: "absolute",
    top: (PANEL_TOP - HANDLE.height) / 2,
    left: "50%",
    marginLeft: -HANDLE.width / 2,
    ...HANDLE,
    borderRadius: HANDLE.height / 2,
    pointerEvents: "none",
    backgroundColor: LABEL.tertiary,
  },
});
