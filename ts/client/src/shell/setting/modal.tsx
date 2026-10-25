// @pxl/client/shell/setting/modal.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Panel } from "@/shell/ui/panel.tsx";
import { PanelBar, PanelBody, PanelScroll } from "@/shell/ui/panel.tsx";
import { Segmented } from "@/shell/ui/segmented.tsx";
import { hairline } from "@/shell/ui/theme.ts";
import { SettingsSection } from "./section.tsx";
import { settingText } from "./row.tsx";
import { LanguageSection } from "@/shell/setting/info/language.tsx";
import { DisclaimerSection } from "@/shell/setting/info/disclaimer.tsx";
import { AboutSection } from "@/shell/setting/info/about.tsx";
import { AttributionSection } from "@/shell/setting/info/attribution.tsx";

const enum SettingsTab {
  Options = 0,
  Info = 1,
}

function Separator() {
  return <View style={styles.separator} />;
}

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<SettingsTab>(SettingsTab.Options);

  return (
    <Panel label={t("settings.title")} onClose={onClose}>
      <PanelBody
        bar={
          <PanelBar onClose={onClose}>
            <Segmented
              value={tab}
              onChange={setTab}
              segments={[
                { value: SettingsTab.Options, label: t("settings.tabs.options") },
                { value: SettingsTab.Info, label: t("settings.tabs.info") },
              ]}
            />
          </PanelBar>
        }
      >
        {tab === SettingsTab.Options
          ? (
            <PanelScroll key={SettingsTab.Options}>
              <Text style={settingText.faint}>{t("settings.cost_legend")}</Text>
              <Separator />
              <SettingsSection section="display" />
              <SettingsSection section="navigation" />
              <SettingsSection section="camera" />
              <SettingsSection section="lighting" />
              <SettingsSection section="debug" />
            </PanelScroll>
          )
          : (
            <PanelScroll key={SettingsTab.Info}>
              <LanguageSection />
              <Separator />
              <DisclaimerSection />
              <AttributionSection />
              <AboutSection />
            </PanelScroll>
          )}
      </PanelBody>
    </Panel>
  );
}

const styles = StyleSheet.create({
  separator: { height: 1, backgroundColor: hairline },
});
