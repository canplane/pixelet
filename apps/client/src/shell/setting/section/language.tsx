// @pxl/client/shell/setting/section/language.tsx

import { useTranslation } from "react-i18next";

import { Section, SettingChoiceRow, SettingChoices } from "@/shell/setting/row.tsx";
import { type Language, LANGUAGE_NAMES } from "@/shell/locale/i18n.ts";

export function LanguageSection() {
  const { t, i18n } = useTranslation();
  return (
    <Section title={t("settings.language.section_title")} icon="globe">
      <SettingChoices>
        {(Object.keys(LANGUAGE_NAMES) as Language[]).map((code) => (
          <SettingChoiceRow
            key={code}
            label={LANGUAGE_NAMES[code]}
            selected={i18n.resolvedLanguage === code}
            onSelect={() => i18n.changeLanguage(code)}
          />
        ))}
      </SettingChoices>
    </Section>
  );
}
