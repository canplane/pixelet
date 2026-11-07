// @pxl/client/ui/setting/modal.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Panel } from "@/ui/component/panel.tsx";
import { Tabbed } from "@/ui/component/tabbed.tsx";
import { hairline } from "@/ui/component/theme.ts";
import { SettingsSection } from "./section.tsx";
import { settingText } from "./row.tsx";
import { LanguageSection } from "@/ui/setting/info/language.tsx";
import { DisclaimerSection } from "@/ui/setting/info/disclaimer.tsx";
import { AboutSection } from "@/ui/setting/info/about.tsx";
import { AttributionSection } from "@/ui/setting/info/attribution.tsx";

const OPTIONS = 0;
const INFO = 1;

function Separator() {
  return <View style={styles.separator} />;
}

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  return (
    <Panel label={t("settings.title")} onClose={onClose}>
      <Tabbed
        onClose={onClose}
        tabs={[
          { value: OPTIONS, label: t("settings.tabs.options") },
          { value: INFO, label: t("settings.tabs.info") },
        ]}
      >
        {(tab) =>
          tab === OPTIONS ? (
            <>
              <Text style={settingText.faint}>{t("settings.cost_legend")}</Text>
              <Separator />
              <SettingsSection section="display" />
              <SettingsSection section="camera" />
              <SettingsSection section="lighting" />
              <SettingsSection section="debug" />
            </>
          ) : (
            <>
              <LanguageSection />
              <Separator />
              <DisclaimerSection />
              <AttributionSection />
              <AboutSection />
            </>
          )
        }
      </Tabbed>
    </Panel>
  );
}

const styles = StyleSheet.create({
  separator: { height: 1, backgroundColor: hairline },
});
