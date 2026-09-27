// @pxl/web/feature/setting/section/language.tsx

import { Check, Languages } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { SectionTitle } from "@/feature/setting/row.tsx";
import { color, textStyle } from "@/shared/ui/theme.ts";

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
          return (
            <Pressable
              key={code}
              onPress={() => i18n.changeLanguage(code)}
              accessibilityRole="radio"
              aria-checked={on}
              style={styles.choice}
            >
              <Text style={styles.label}>{t(labelKey)}</Text>
              {on && <Check size={18} strokeWidth={2.5} color={color.chosen} />}
            </Pressable>
          );
        })}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  section: { gap: 4 },
  // A row reaches past the panel's padding to its edge, as a setting's does.
  choices: { marginHorizontal: -12 },
  choice: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", height: 40, paddingHorizontal: 12 },
  label: { ...textStyle.body, color: color.text },
});
