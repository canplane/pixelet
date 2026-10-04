// @pxl/web/shared/ui/panel.tsx

import { createContext, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ScrollView, StyleSheet, View, type ViewProps } from "react-native";

import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { chrome, color, material } from "@/shared/ui/theme.ts";
import { useSafeArea } from "@platform/safe-area.ts";

export const PANEL_CONTROL = chrome.target;
/** iOS 26 Maps' sheet inset, for the top row's controls and the content's sides. */
export const PANEL_EDGE = 16;
/** Above the top row: the band the sheet's handle stands in. */
export const PANEL_TOP = 20;

export const PANEL_SECTION_GAP = 24;
export const PANEL_HEAD_HEIGHT = PANEL_TOP + PANEL_CONTROL + PANEL_EDGE;
export const PANEL_CONTENT_TOP = PANEL_TOP + PANEL_CONTROL + PANEL_SECTION_GAP;

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

/** The top row: the sheet's own control where a title would stand — its tabs or
 * its search field — and close. The panel's `label` names the sheet. */
export function PanelBar({ onClose, children, ...rest }: ViewProps & { onClose: () => void; children: ReactNode }) {
  return (
    <View {...rest} style={styles.bar}>
      <View style={styles.barMain}>{children}</View>
      <PanelClose onClose={onClose} />
    </View>
  );
}

export function PanelBody({ bar, children }: { bar: ReactNode; children: ReactNode }) {
  return (
    <View style={styles.body}>
      <View style={styles.scroll}>{children}</View>
      <Backing fill={color.ground} material={material.bar} place={BackingPlace.OverContent} style={styles.head}>
        {bar}
      </Backing>
    </View>
  );
}

function PanelClose({ onClose }: { onClose: () => void }) {
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
  bar: { marginTop: PANEL_TOP, height: PANEL_CONTROL, paddingHorizontal: PANEL_EDGE, flexDirection: "row", alignItems: "center", gap: chrome.controlGap },
  barMain: { flex: 1, height: PANEL_CONTROL, justifyContent: "center" },
  body: { flex: 1 },
  head: { position: "absolute", top: 0, left: 0, right: 0, height: PANEL_HEAD_HEIGHT },
  tabScroll: { flex: 1 },
  tabContent: { paddingHorizontal: PANEL_EDGE, paddingTop: PANEL_CONTENT_TOP, gap: PANEL_SECTION_GAP },
  scroll: { flex: 1, overflow: "hidden" },
});
