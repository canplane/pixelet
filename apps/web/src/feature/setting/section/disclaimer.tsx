// @pxl/web/feature/setting/section/disclaimer.tsx

import { Trans, useTranslation } from "react-i18next";
import { Linking, Text, View } from "react-native";

import { SectionTitle, settingText } from "@/feature/setting/row.tsx";

export function DisclaimerSection() {
  const { t } = useTranslation();
  return (
    <View style={{ gap: 4 }}>
      <SectionTitle>{t("settings.disclaimer.section_title")}</SectionTitle>
      <View style={{ gap: 8, paddingVertical: 4 }}>
        <Text style={settingText.small}>{t("settings.disclaimer.demo_notice")}</Text>
        <Text style={settingText.small}>{t("settings.disclaimer.coverage_notice")}</Text>
        <Text style={settingText.small}>{t("settings.disclaimer.storage_notice")}</Text>
        <Text style={settingText.small}>{t("settings.disclaimer.location_notice")}</Text>
        <Text style={settingText.small}>
          <Trans
            i18nKey="settings.disclaimer.character_notice"
            components={{ kenney: <Text style={settingText.link} onPress={() => Linking.openURL("https://kenney.nl/")} /> }}
          />
        </Text>
      </View>
    </View>
  );
}
