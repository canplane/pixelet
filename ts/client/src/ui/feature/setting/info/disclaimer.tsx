// @pxl/client/ui/feature/setting/info/disclaimer.tsx

import { useTranslation } from "react-i18next";
import { Text } from "react-native";

import { Prose, Section, SETTING_TEXT } from "@/ui/feature/setting/row.tsx";

export function DisclaimerSection() {
  const { t } = useTranslation();
  return (
    <Section title={t("settings.disclaimer.section_title")}>
      <Prose>
        <Text style={SETTING_TEXT.small}>
          {t("settings.disclaimer.demo_notice")}
        </Text>
        <Text style={SETTING_TEXT.small}>
          {t("settings.disclaimer.coverage_notice")}
        </Text>
        <Text style={SETTING_TEXT.small}>
          {t("settings.disclaimer.storage_notice")}
        </Text>
        <Text style={SETTING_TEXT.small}>
          {t("settings.disclaimer.location_notice")}
        </Text>
      </Prose>
    </Section>
  );
}
