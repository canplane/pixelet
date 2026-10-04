// @pxl/web/feature/setting/modal.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ScrollView, StyleSheet, Text, View } from "react-native";

import type { BaseModalProps } from "@/app/type/ui.type.ts";

import { useSafeArea } from "@platform/safe-area.ts";
import { Panel } from "@platform/panel.tsx";
import { PANEL_EDGE, PANEL_SECTION_GAP, PANEL_TABS_ROOM, PanelBody, PanelHeader } from "@/shared/ui/panel.tsx";
import { Segmented } from "@platform/segmented.tsx";
import { hairline } from "@/shared/ui/theme.ts";
import { SettingsSection } from "./section.tsx";
import { settingText } from "./row.tsx";
import { LanguageSection } from "./section/language.tsx";
import { DisclaimerSection } from "./section/disclaimer.tsx";
import { AboutSection } from "./section/about.tsx";
import { AttributionSection } from "./section/attribution.tsx";

export const enum SettingsTab {
  Effects = 0,
  Info = 1,
}

function Separator() {
  return <View style={styles.separator} />;
}

// One scroller per tab, mounted fresh: a shared one kept its offset across a switch.
function TabScroll({ children }: { children: React.ReactNode }) {
  const { bottom } = useSafeArea();
  return (
    <ScrollView style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingBottom: bottom + PANEL_EDGE }]}>
      {children}
    </ScrollView>
  );
}

export function SettingsModal(props: BaseModalProps) {
  const { onClose } = props;
  const { t } = useTranslation();
  const [tab, setTab] = useState<SettingsTab>(SettingsTab.Effects);

  return (
    <Panel label={t("settings.title")} onClose={onClose}>
      <PanelBody
        header={<PanelHeader title={t("settings.title")} closeLabel={t("settings.close")} onClose={onClose} />}
        tabs={
          <Segmented
            value={tab}
            onChange={setTab}
            segments={[
              { value: SettingsTab.Effects, label: t("settings.tabs.effects") },
              { value: SettingsTab.Info, label: t("settings.tabs.info") },
            ]}
          />
        }
      >
        {tab === SettingsTab.Effects
          ? (
            <TabScroll key={SettingsTab.Effects}>
              <Text style={settingText.faint}>{t("settings.cost_legend")}</Text>
              <Separator />
              <SettingsSection section="display" />
              <SettingsSection section="navigation" />
              <SettingsSection section="camera" />
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
      </PanelBody>
    </Panel>
  );
}

const styles = StyleSheet.create({
  scroll: { flex: 1 },
  scrollContent: { paddingHorizontal: PANEL_EDGE, paddingTop: PANEL_TABS_ROOM, gap: PANEL_SECTION_GAP },
  separator: { height: 1, backgroundColor: hairline },
});
