// @pxl/web/shared/ui/panel.tsx

import { createContext, type ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { chrome, color, textStyle } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

/** Height of the controls on a sheet (close button, search field, segmented
 * control): 44, iOS's least target, a step under a map control's 48. */
export const PANEL_CONTROL = 44;
/** Top row height, the same as an iOS navigation bar. */
const ROW = 44;
/** Distance of the top row's controls from the sheet's side and top edges,
 * and the side inset of a panel's content: iOS 26's, as its Maps' sheet
 * stands its search field and close. The drag handle, 5 from the top and 5
 * tall, leaves 6 above the controls. */
export const PANEL_EDGE = 16;

/** A sheet's top row. */
export const PANEL_HEADER = {
  marginTop: PANEL_EDGE - (ROW - PANEL_CONTROL) / 2,
  height: ROW,
  paddingHorizontal: PANEL_EDGE,
  flexDirection: "row",
  alignItems: "center",
} as const;

/** Gap between the top row's controls and the floating tabs under them: the
 * one between any two controls. */
const STACK_GAP = chrome.controlGap;
/** Gap between the tabs and the first row of content. */
const CONTENT_GAP = 16;
/** Height of the header area: PANEL_EDGE above and below the round
 * controls. Content scrolls only below it, and a sheet is dragged by it. */
export const PANEL_HEAD_HEIGHT = PANEL_EDGE + PANEL_CONTROL + PANEL_EDGE;
/** Top of the floating tabs, measured from the sheet's top: STACK_GAP under
 * the top row's controls, so they straddle the header's bottom edge. */
const TABS_TOP = PANEL_EDGE + PANEL_CONTROL + STACK_GAP;
/** Top padding a panel's scroll content needs to start below the part of
 * the tabs that hangs into the scroll area. */
export const PANEL_TABS_ROOM = TABS_TOP + PANEL_CONTROL + CONTENT_GAP - PANEL_HEAD_HEIGHT;

/** Corner radius of a sheet's top corners: PANEL_EDGE plus the controls' radius,
 * so the corners are concentric with the round controls in the top row. */
export const SHEET_RADIUS = PANEL_EDGE + PANEL_CONTROL / 2;

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

/** A panel's body: the header area (`header`, PANEL_HEAD_HEIGHT tall) and
 * under it the scroll area (`children`), so content never scrolls behind the
 * header. The tabs float across the boundary, over the content that moves
 * behind their lower part; scroll content under tabs pads its top by
 * PANEL_TABS_ROOM. */
export function PanelBody({ header, tabs, children }: { header: ReactNode; tabs?: ReactNode; children: ReactNode }) {
  return (
    <View style={styles.body}>
      <View style={styles.head}>{header}</View>
      <View style={styles.scroll}>{children}</View>
      {tabs && (
        <Backing fill={color.paper} place={BackingPlace.OverContent} style={styles.tabs}>
          {tabs}
        </Backing>
      )}
    </View>
  );
}

/** A panel's close button: a round glass button with an X, as on iOS 26
 * sheets, a sheet control's size. */
export function PanelClose({ label, onClose }: { label: string; onClose: () => void }) {
  return <CircularButton glyph="xmark" size={PANEL_CONTROL} place={BackingPlace.OnSheet} onPress={onClose} label={label} />;
}

const styles = StyleSheet.create({
  headerRow: { ...PANEL_HEADER, justifyContent: "flex-end" },
  // Spans the whole row so the title is centered on the sheet, not on the
  // space left of the close button.
  title: { position: "absolute", left: 0, right: 0, textAlign: "center", pointerEvents: "none", ...textStyle.title, color: LABEL.primary },
  body: { flex: 1 },
  head: { height: PANEL_HEAD_HEIGHT },
  scroll: { flex: 1, overflow: "hidden" },
  tabs: {
    position: "absolute",
    top: TABS_TOP,
    left: PANEL_EDGE,
    right: PANEL_EDGE,
    height: PANEL_CONTROL,
    borderRadius: PANEL_CONTROL / 2,
    justifyContent: "center",
    boxShadow: "0 2px 8px rgba(0, 0, 0, 0.08)",
  },
});
