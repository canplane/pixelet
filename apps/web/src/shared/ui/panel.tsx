// @pxl/web/shared/ui/panel.tsx

import { X } from "lucide-react-native";
import { createContext, type ReactNode, useContext } from "react";
import { Pressable, StyleSheet, Text, useWindowDimensions, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { SLIDE, SLIDE_AWAY } from "@/shared/ui/motion/transition.ts";
import { alpha, color, font, hairline } from "@/shared/ui/theme.ts";

/** Under this, a panel is the whole screen's width. */
const NARROW_PX = 640;
export const PANEL_HEADER_PX = 56;

interface PanelProps {
  side: "left" | "right";
  /** Its width where the screen is wide enough to leave the map beside it. */
  width: number;
  /** What assistive technology announces it as. */
  label: string;
  onClose: () => void;
  /** A press on the map beside it closes it rather than reaching the map. */
  dismissOnOutside?: boolean;
  children: ReactNode;
}

/**
 * Whether the panel's host still shows it, and what to call once it has slid
 * away: a host drops a panel only after that (shared/ui/motion/presence.tsx),
 * and the host is the one place that knows, so it says so here rather than
 * through every panel's props.
 */
export const PanelPresence = createContext<{ open: boolean; gone: () => void }>({ open: true, gone: () => {} });

/**
 * A panel along one side of the screen, over the live map, sliding in from
 * that side and back out. Not a modal and
 * not dimmed: the panels exist to be used while looking at the world, and a
 * device's modal is a window of its own that the map could not be touched
 * through at all. Where tapping away is meant to leave it, the map beside it
 * takes the tap for that instead.
 */
export function Panel({ side, width, label, onClose, dismissOnOutside, children }: PanelProps) {
  const window = useWindowDimensions();
  const { open, gone } = useContext(PanelPresence);
  const narrow = window.width < NARROW_PX;
  const shown = narrow ? window.width : width;
  // Past its own edge and its shadow, off the side it stands on.
  const away = { translateX: (side === "left" ? -1 : 1) * (shown + 16) };

  return (
    <View style={[styles.layer, open ? styles.pass : styles.block]}>
      {dismissOnOutside && <Pressable accessible={false} onPress={onClose} style={StyleSheet.absoluteFill} />}
      <Motion
        from={away}
        to={open ? {} : away}
        transition={open ? SLIDE : SLIDE_AWAY}
        onSettled={open ? undefined : gone}
        style={[styles.panel, side === "left" ? styles.left : styles.right, { width: shown }]}
      >
        <View accessibilityViewIsModal accessibilityLabel={label} style={styles.fill}>
          {children}
        </View>
      </Motion>
    </View>
  );
}

/** A panel's title row, under the notch, with the way out at its end. */
export function PanelHeader({ title, closeLabel, onClose }: { title: string; closeLabel: string; onClose: () => void }) {
  const { top } = useSafeArea();
  return (
    <View style={[styles.header, { paddingTop: top }]}>
      <View style={styles.headerRow}>
        <Text accessibilityRole="header" style={styles.title}>{title}</Text>
        <Pressable onPress={onClose} accessibilityRole="button" accessibilityLabel={closeLabel} style={styles.close}>
          <X size={20} color={color.textMuted} />
        </Pressable>
      </View>
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
  panel: { position: "absolute", top: 0, bottom: 0, backgroundColor: color.paper, boxShadow: "0 10px 15px rgba(0, 0, 0, 0.1)" },
  left: { left: 0, borderRightWidth: 1, borderRightColor: hairline },
  right: { right: 0, borderLeftWidth: 1, borderLeftColor: hairline },
  header: { borderBottomWidth: 1, borderBottomColor: alpha(color.text, 0.06) },
  headerRow: { height: PANEL_HEADER_PX, flexDirection: "row", alignItems: "center", gap: 8, paddingHorizontal: 20 },
  title: { flex: 1, fontFamily: font.family, fontSize: 17, fontWeight: "600", color: color.text },
  close: { width: 42, height: 42, marginRight: -10, borderRadius: 21, alignItems: "center", justifyContent: "center" },
});
