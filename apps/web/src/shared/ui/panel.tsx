// @pxl/web/shared/ui/panel.tsx

import { X } from "lucide-react-native";
import { createContext, type ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { Glass } from "@platform/glass.tsx";
import { GlassVariant } from "@/shared/ui/glass-variant.ts";
import { color, textStyle } from "@/shared/ui/theme.ts";

/** Height of the controls on a sheet (close button, search field, segmented
 * control); the same as an iOS search bar. */
export const PANEL_CONTROL = 36;
/** Top row height, the same as an iOS navigation bar. */
const ROW = 44;
/** Distance of the top row's controls from the sheet's side and top edges.
 * iOS draws the sheet's drag handle 5pt from the top and 5pt tall, so this
 * leaves 10pt between the handle and the controls. */
const EDGE = 20;

/** A sheet's top row. No divider under it, as on iOS 26 sheets. */
export const PANEL_HEADER = {
  marginTop: EDGE - (ROW - PANEL_CONTROL) / 2,
  height: ROW,
  paddingHorizontal: EDGE,
  flexDirection: "row",
  alignItems: "center",
} as const;

/** Corner radius of a sheet's top corners: EDGE plus the controls' radius,
 * so the corners are concentric with the round controls in the top row. */
export const SHEET_RADIUS = EDGE + PANEL_CONTROL / 2;

/** Props of a panel, a bottom sheet over the map (platform/web/panel.tsx,
 * and the device app's platform/panel.tsx). */
export interface PanelProps {
  /** What assistive technology announces it as. */
  label: string;
  onClose: () => void;
  /** Open at half height instead of full height. Used by panels opened
   * from the bottom of the screen; panels opened from the top open full. */
  half?: boolean;
  children: ReactNode;
}

/**
 * Whether the panel's host still shows it, and what to call once it has slid
 * away: a host drops a panel only after that (shared/ui/motion/presence.tsx),
 * and the host is the one place that knows, so it says so here rather than
 * through every panel's props.
 */
export const PanelPresence = createContext<{ open: boolean; gone: () => void }>({ open: true, gone: () => {} });

/** A panel's title row: centered title, close button at the end. No
 * safe-area padding, since a sheet never reaches the notch. */
export function PanelHeader({ title, closeLabel, onClose }: { title: string; closeLabel: string; onClose: () => void }) {
  return (
    <View style={styles.headerRow}>
      <Text accessibilityRole="header" style={styles.title}>{title}</Text>
      <PanelClose label={closeLabel} onClose={onClose} />
    </View>
  );
}

/** A panel's close button: a round glass button with an X, as on iOS 26
 * sheets. */
export function PanelClose({ label, onClose }: { label: string; onClose: () => void }) {
  return (
    <Pressable onPress={onClose} accessibilityRole="button" accessibilityLabel={label} style={styles.close}>
      <Glass fill={color.faint} variant={GlassVariant.Clear} interactive style={styles.closeFace}>
        <X size={18} color={color.text} />
      </Glass>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  headerRow: { ...PANEL_HEADER, justifyContent: "flex-end" },
  // Spans the whole row so the title is centered on the sheet, not on the
  // space left of the close button.
  title: { position: "absolute", left: 0, right: 0, textAlign: "center", pointerEvents: "none", ...textStyle.title, color: color.text },
  close: { width: PANEL_CONTROL, height: PANEL_CONTROL, borderRadius: PANEL_CONTROL / 2 },
  closeFace: { flex: 1, borderRadius: PANEL_CONTROL / 2, alignItems: "center", justifyContent: "center" },
});
