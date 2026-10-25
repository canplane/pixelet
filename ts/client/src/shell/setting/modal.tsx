// @pxl/client/shell/setting/modal.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Panel } from "@/shell/ui/panel.tsx";
import { PanelTabs } from "@/shell/ui/panel-tabs.tsx";
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

  return (
    <Panel label={t("settings.title")} onClose={onClose}>
      <PanelTabs
        onClose={onClose}
        tabs={[
          { value: SettingsTab.Options, label: t("settings.tabs.options") },
          { value: SettingsTab.Info, label: t("settings.tabs.info") },
        ]}
      >
        {(tab) =>
          tab === SettingsTab.Options
            ? (
              <>
                <Text style={settingText.faint}>
                  {t("settings.cost_legend")}
                </Text>
                <Separator />
                <SettingsSection section="display" />
                <SettingsSection section="navigation" />
                <SettingsSection section="camera" />
                <SettingsSection section="lighting" />
                <SettingsSection section="debug" />
              </>
            )
            : (
              <>
                <LanguageSection />
                <Separator />
                <DisclaimerSection />
                <AttributionSection />
                <AboutSection />
              </>
            )}
      </PanelTabs>
    </Panel>
  );
}

const styles = StyleSheet.create({
  separator: { height: 1, backgroundColor: hairline },
});
