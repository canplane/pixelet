// @pxl/web/shared/ui/panel.tsx

import { X } from "lucide-react-native";
import type { ReactNode } from "react";
import { Pressable, StyleSheet, Text, useWindowDimensions, View } from "react-native";

import { useSafeArea } from "@platform/safe-area.ts";
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
 * A panel along one side of the screen, over the live map. Not a modal and
 * not dimmed: the panels exist to be used while looking at the world, and a
 * device's modal is a window of its own that the map could not be touched
 * through at all. Where tapping away is meant to leave it, the map beside it
 * takes the tap for that instead.
 */
export function Panel({ side, width, label, onClose, dismissOnOutside, children }: PanelProps) {
  const window = useWindowDimensions();
  const narrow = window.width < NARROW_PX;

  return (
    <View style={styles.layer}>
      {dismissOnOutside && <Pressable accessible={false} onPress={onClose} style={StyleSheet.absoluteFill} />}
      <View
        accessibilityViewIsModal
        accessibilityLabel={label}
        style={[styles.panel, side === "left" ? styles.left : styles.right, { width: narrow ? window.width : width }]}
      >
        {children}
      </View>
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
          <X size={20} color={color.mutedForeground} />
        </Pressable>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  // Over the map and its toasts.
  layer: { position: "absolute", top: 0, right: 0, bottom: 0, left: 0, zIndex: 70, pointerEvents: "box-none" },
  panel: { position: "absolute", top: 0, bottom: 0, backgroundColor: color.background, boxShadow: "0 10px 15px rgba(0, 0, 0, 0.1)" },
  left: { left: 0, borderRightWidth: 1, borderRightColor: hairline },
  right: { right: 0, borderLeftWidth: 1, borderLeftColor: hairline },
  header: { borderBottomWidth: 1, borderBottomColor: alpha(color.foreground, 0.06) },
  headerRow: { height: PANEL_HEADER_PX, flexDirection: "row", alignItems: "center", gap: 8, paddingHorizontal: 20 },
  title: { flex: 1, fontFamily: font.family, fontSize: 17, fontWeight: "600", color: color.foreground },
  close: { width: 42, height: 42, marginRight: -10, borderRadius: 21, alignItems: "center", justifyContent: "center" },
});
