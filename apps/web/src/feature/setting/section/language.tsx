// @pxl/web/feature/setting/section/language.tsx

import { Languages } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { SectionTitle } from "@/feature/setting/row.tsx";
import { alpha, color, font } from "@/shared/ui/theme.ts";

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
              style={[styles.choice, on ? styles.on : styles.off]}
            >
              <Text style={[styles.label, on && styles.labelOn]}>{t(labelKey)}</Text>
            </Pressable>
          );
        })}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  section: { gap: 4 },
  choices: { flexDirection: "row", gap: 6 },
  choice: { flex: 1, height: 32, alignItems: "center", justifyContent: "center", borderRadius: 16, borderWidth: 1, paddingHorizontal: 8 },
  on: { borderColor: color.primary, backgroundColor: alpha(color.primary, 0.1) },
  off: { borderColor: "transparent", backgroundColor: alpha(color.accent, 0.3) },
  label: { fontFamily: font.family, fontSize: 13, fontWeight: "500", color: color.mutedForeground },
  labelOn: { color: color.foreground },
});
