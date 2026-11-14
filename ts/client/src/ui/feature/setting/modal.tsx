// @pxl/client/ui/feature/setting/modal.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Panel } from "@/ui/shared/component/panel.tsx";
import { Tabbed } from "@/ui/shared/component/tabbed.tsx";
import { HAIRLINE } from "@/ui/shared/component/theme.ts";
import { SettingsSection } from "./section.tsx";
import { SETTING_TEXT } from "./row.tsx";
import { LanguageSection } from "@/ui/feature/setting/info/language.tsx";
import { DisclaimerSection } from "@/ui/feature/setting/info/disclaimer.tsx";
import { AboutSection } from "@/ui/feature/setting/info/about.tsx";
import { AttributionSection } from "@/ui/feature/setting/info/attribution.tsx";

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
              <Text style={SETTING_TEXT.faint}>
                {t("settings.cost_legend")}
              </Text>
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
  separator: { height: 1, backgroundColor: HAIRLINE },
});
