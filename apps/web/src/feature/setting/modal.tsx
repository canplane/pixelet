// @pxl/web/feature/setting/modal.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ScrollView, StyleSheet, Text, View } from "react-native";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { Info, Sparkles } from "lucide-react-native";

import { useSafeArea } from "@platform/safe-area.ts";
import { Panel, PanelHeader, Side } from "@/shared/ui/panel.tsx";
import { Segmented } from "@/shared/ui/segmented.tsx";
import { color, hairline } from "@/shared/ui/theme.ts";
import { SettingsSection } from "./section.tsx";
import { settingText } from "./row.tsx";
import { LanguageSection } from "./section/language.tsx";
import { DisclaimerSection } from "./section/disclaimer.tsx";
import { AboutSection } from "./section/about.tsx";
import { AttributionSection } from "./section/attribution.tsx";

/** The sheet's two tabs. */
export const enum SettingsTab {
  Effects = 0,
  Info = 1,
}

function Separator() {
  return <View style={styles.separator} />;
}

// Each tab scrolls on its own, and only the tab shown is mounted. One
// scroller around both kept its offset across a switch, so a long scroll
// down the effects list opened the info tab already scrolled past its top;
// a scroller mounted fresh starts at zero every time.
function TabScroll({ children }: { children: React.ReactNode }) {
  const { bottom } = useSafeArea();
  return (
    <ScrollView style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingBottom: bottom + 16 }]}>
      {children}
    </ScrollView>
  );
}

/** The settings, beside the map they change: the map stays in reach, so an
 * effect can be switched and looked at without closing anything. */
export function SettingsModal(props: BaseModalProps) {
  const { onClose } = props;
  const { t } = useTranslation();
  const [tab, setTab] = useState<SettingsTab>(SettingsTab.Effects);

  return (
    <Panel side={Side.Left} width={360} label={t("settings.title")} onClose={onClose}>
      <PanelHeader title={t("settings.title")} closeLabel={t("settings.close")} onClose={onClose} />

      <View style={styles.tabs}>
        <Segmented
          value={tab}
          onChange={setTab}
          segments={[
            { value: SettingsTab.Effects, label: t("settings.tabs.effects"), icon: <Sparkles size={14} color={color.textMuted} /> },
            { value: SettingsTab.Info, label: t("settings.tabs.info"), icon: <Info size={14} color={color.textMuted} /> },
          ]}
        />
      </View>

      {tab === SettingsTab.Effects
        ? (
          <TabScroll key={SettingsTab.Effects}>
            <Text style={settingText.faint}>{t("settings.cost_legend")}</Text>
            <Separator />
            <SettingsSection section="display" />
            <SettingsSection section="camera" />
            <SettingsSection section="editing" />
            <SettingsSection section="lighting" />
            <SettingsSection section="quality" />
            <SettingsSection section="debug" />
          </TabScroll>
        )
        : (
          <TabScroll key={SettingsTab.Info}>
            <LanguageSection />
            <Separator />
            <DisclaimerSection />
            <AttributionSection />
            <AboutSection />
          </TabScroll>
        )}
    </Panel>
  );
}

const styles = StyleSheet.create({
  tabs: { paddingHorizontal: 20, paddingTop: 16 },
  scroll: { flex: 1 },
  scrollContent: { paddingHorizontal: 20, paddingTop: 16, gap: 16 },
  separator: { height: 1, backgroundColor: hairline },
});
