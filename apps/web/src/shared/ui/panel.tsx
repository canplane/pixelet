// @pxl/web/shared/ui/panel.tsx

import { createContext, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ScrollView, StyleSheet, Text, View } from "react-native";

import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { chrome, color, material, textStyle } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";
import { useSafeArea } from "@platform/safe-area.ts";

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

export const PANEL_SECTION_GAP = 24;
export const PANEL_HEAD_HEIGHT = PANEL_EDGE + PANEL_CONTROL + PANEL_EDGE;
export const PANEL_CONTENT_TOP = PANEL_EDGE + PANEL_CONTROL + PANEL_SECTION_GAP;
const TABS_ROW = chrome.controlGap + PANEL_CONTROL;
const TABS_ROOM = PANEL_CONTENT_TOP + TABS_ROW;

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

export function PanelHeader({ title, onClose }: { title: string; onClose: () => void }) {
  return (
    <View style={styles.headerRow}>
      <Text accessibilityRole="header" style={styles.title}>{title}</Text>
      <PanelClose onClose={onClose} />
    </View>
  );
}

export function PanelBody({ header, tabs, children }: { header: ReactNode; tabs?: ReactNode; children: ReactNode }) {
  return (
    <View style={styles.body}>
      <View style={styles.scroll}>{children}</View>
      {
        // The bar holds the tabs whole, as a navigation bar holds its palette.
      }
      <Backing fill={color.ground} material={material.bar} place={BackingPlace.OverContent} style={[styles.head, tabs ? styles.headTabs : null]}>
        {header}
        {tabs && <View style={styles.tabs}>{tabs}</View>}
      </Backing>
    </View>
  );
}

export function PanelClose({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return <CircularButton glyph="xmark" size={PANEL_CONTROL} place={BackingPlace.OnSheet} onPress={onClose} label={t("close")} />;
}

/** A tab's scroller under the tabs; keyed per tab, since a shared one kept its offset across a switch. */
export function PanelScroll({ children }: { children: ReactNode }) {
  const { bottom } = useSafeArea();
  return (
    <ScrollView style={styles.tabScroll} contentContainerStyle={[styles.tabContent, { paddingBottom: bottom + PANEL_EDGE }]}>
      {children}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  headerRow: { ...PANEL_HEADER, justifyContent: "flex-end" },
  // Spans the row so the title centers on the sheet, not beside the close button.
  title: { position: "absolute", left: 0, right: 0, textAlign: "center", pointerEvents: "none", ...textStyle.title, color: LABEL.primary },
  body: { flex: 1 },
  head: { position: "absolute", top: 0, left: 0, right: 0, height: PANEL_HEAD_HEIGHT },
  headTabs: { height: PANEL_HEAD_HEIGHT + TABS_ROW },
  tabScroll: { flex: 1 },
  tabContent: { paddingHorizontal: PANEL_EDGE, paddingTop: TABS_ROOM, gap: PANEL_SECTION_GAP },
  scroll: { flex: 1, overflow: "hidden" },
  tabs: { marginTop: chrome.controlGap, marginHorizontal: PANEL_EDGE, height: PANEL_CONTROL, justifyContent: "center" },
});
