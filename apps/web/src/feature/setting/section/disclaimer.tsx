// @pxl/web/feature/setting/section/disclaimer.tsx

import { Trans, useTranslation } from "react-i18next";
import { Text } from "react-native";

import { Link, Prose, Section, settingText } from "@/feature/setting/row.tsx";

export function DisclaimerSection() {
  const { t } = useTranslation();
  return (
    <Section title={t("settings.disclaimer.section_title")}>
      <Prose>
        <Text style={settingText.small}>{t("settings.disclaimer.demo_notice")}</Text>
        <Text style={settingText.small}>{t("settings.disclaimer.coverage_notice")}</Text>
        <Text style={settingText.small}>{t("settings.disclaimer.storage_notice")}</Text>
        <Text style={settingText.small}>{t("settings.disclaimer.location_notice")}</Text>
        <Text style={settingText.small}>
          <Trans
            i18nKey="settings.disclaimer.character_notice"
            components={{ kenney: <Link href="https://kenney.nl/" /> }}
          />
        </Text>
      </Prose>
    </Section>
  );
}
