// @pxl/web/feature/setting/section/language.tsx

import { Languages } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import { SectionTitle, SettingChoiceRow } from "@/feature/setting/row.tsx";

const SUPPORTED_LANGUAGES = [
  { code: "en", labelKey: "settings.language.english" },
  { code: "ko", labelKey: "settings.language.korean" },
] as const;

export function LanguageSection() {
  const { t, i18n } = useTranslation();

  return (
    <View style={styles.section}>
      <SectionTitle icon={Languages}>{t("settings.language.section_title")}</SectionTitle>
      <View style={styles.choices}>
        {SUPPORTED_LANGUAGES.map(({ code, labelKey }) => {
          const on = i18n.resolvedLanguage === code;
          return <SettingChoiceRow key={code} label={t(labelKey)} selected={on} onSelect={() => i18n.changeLanguage(code)} />;
        })}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  section: { gap: 4 },
  // A row reaches past the panel's padding to its edge, as a setting's does.
  choices: { marginHorizontal: -12 },
});
