// @pxl/web/shared/ui/panel.tsx

import { createContext, type ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { chrome, color, material, textStyle } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

export const PANEL_CONTROL = chrome.target;
/** iOS 26 Maps' sheet inset, for the top row's controls and the content's sides. */
export const PANEL_EDGE = 16;

export const PANEL_HEADER = {
  marginTop: PANEL_EDGE,
  height: PANEL_CONTROL,
  paddingHorizontal: PANEL_EDGE,
  flexDirection: "row",
  alignItems: "center",
} as const;

const STACK_GAP = chrome.controlGap;
export const PANEL_SECTION_GAP = 24;
const CONTENT_GAP = PANEL_SECTION_GAP;
export const PANEL_HEAD_HEIGHT = PANEL_EDGE + PANEL_CONTROL + PANEL_EDGE;
const TABS_TOP = PANEL_EDGE + PANEL_CONTROL + STACK_GAP;
export const PANEL_CONTENT_TOP = PANEL_EDGE + PANEL_CONTROL + CONTENT_GAP;
export const PANEL_TABS_ROOM = PANEL_CONTENT_TOP + STACK_GAP + PANEL_CONTROL;

/** Concentric with the round controls in the top row. */
export const SHEET_RADIUS = PANEL_EDGE + PANEL_CONTROL / 2;

export interface PanelProps {
  label: string;
  onClose: () => void;
  half?: boolean;
  children: ReactNode;
}

/** A host drops a panel only after `gone`, once it has slid away. */
export const PanelPresence = createContext<{ open: boolean; gone: () => void }>({ open: true, gone: () => {} });

export function PanelHeader({ title, closeLabel, onClose }: { title: string; closeLabel: string; onClose: () => void }) {
  return (
    <View style={styles.headerRow}>
      <Text accessibilityRole="header" style={styles.title}>{title}</Text>
      <PanelClose label={closeLabel} onClose={onClose} />
    </View>
  );
}

export function PanelBody({ header, tabs, children }: { header: ReactNode; tabs?: ReactNode; children: ReactNode }) {
  return (
    <View style={styles.body}>
      <View style={styles.scroll}>{children}</View>
      <Backing fill={color.ground} material={material.bar} place={BackingPlace.OverContent} style={styles.head}>
        {header}
      </Backing>
      {tabs && (
        <Backing fill={color.ground} place={BackingPlace.OverContent} style={styles.tabs}>
          {tabs}
        </Backing>
      )}
    </View>
  );
}

export function PanelClose({ label, onClose }: { label: string; onClose: () => void }) {
  return <CircularButton glyph="xmark" size={PANEL_CONTROL} place={BackingPlace.OnSheet} onPress={onClose} label={label} />;
}

const styles = StyleSheet.create({
  headerRow: { ...PANEL_HEADER, justifyContent: "flex-end" },
  // Spans the row so the title centers on the sheet, not beside the close button.
  title: { position: "absolute", left: 0, right: 0, textAlign: "center", pointerEvents: "none", ...textStyle.title, color: LABEL.primary },
  body: { flex: 1 },
  head: { position: "absolute", top: 0, left: 0, right: 0, height: PANEL_HEAD_HEIGHT },
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
