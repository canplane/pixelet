// @pxl/web/platform/web/panel.tsx

/**
 * A panel on the web: a bottom sheet like the device's (the device app's
 * platform/panel.tsx), over a dimmed map; tapping the map closes it. The web
 * has no system sheet, so it opens at a fixed height (full, or half when
 * `half`) and cannot be dragged.
 *
 * Max width is 540, the width of an iPad form sheet, so rows do not stretch
 * across a desktop screen.
 */

import { useContext } from "react";
import { Pressable, StyleSheet, useWindowDimensions, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { SLIDE, SLIDE_AWAY } from "@/shared/ui/motion/transition.ts";
import { PanelPresence, type PanelProps, SHEET_RADIUS } from "@/shared/ui/panel.tsx";
import { alpha, color } from "@/shared/ui/theme.ts";

const MAX_WIDTH = 540;
/** Gap between the safe-area top and a full-height sheet, as on iOS. */
const WHOLE_GAP = 10;

export function Panel({ label, onClose, half, children }: PanelProps) {
  const window = useWindowDimensions();
  const { top } = useSafeArea();
  const { open, gone } = useContext(PanelPresence);
  const width = Math.min(window.width, MAX_WIDTH);
  const height = half ? window.height / 2 : window.height - top - WHOLE_GAP;
  // Past its own edge and its shadow, below the screen.
  const away = { translateY: height + 16 };

  return (
    <View style={[styles.layer, open ? styles.pass : styles.block]}>
      <Motion from={{ opacity: 0 }} to={{ opacity: open ? 1 : 0 }} transition={open ? SLIDE : SLIDE_AWAY} style={StyleSheet.absoluteFill}>
        <Pressable accessible={false} onPress={onClose} style={styles.dim} />
      </Motion>
      <Motion
        from={away}
        to={open ? {} : away}
        transition={open ? SLIDE : SLIDE_AWAY}
        onSettled={open ? undefined : gone}
        style={[styles.sheet, { width, height, left: (window.width - width) / 2 }]}
      >
        <View accessibilityViewIsModal accessibilityLabel={label} style={styles.fill}>
          {children}
        </View>
      </Motion>
    </View>
  );
}

const styles = StyleSheet.create({
  // Over the map and its toasts.
  layer: { position: "absolute", top: 0, right: 0, bottom: 0, left: 0, zIndex: 70 },
  fill: { flex: 1 },
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
});
