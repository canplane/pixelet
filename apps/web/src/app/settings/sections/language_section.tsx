// @pxl/web/app/settings/sections/language_section.tsx

import { Languages } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/utils/cn.ts";
import { SectionTitle } from "@/app/settings/setting_row.tsx";

const SUPPORTED_LANGUAGES = [
  { code: "en", label_key: "settings.language.english" },
  { code: "ko", label_key: "settings.language.korean" },
] as const;

/** Language switch — replaces the old dev-only LanguageToggleButton (app/dev/language_toggle_button.tsx), never actually mounted anywhere. */
export function LanguageSection() {
  const { t, i18n } = useTranslation();

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Languages}>{t("settings.language.section_title")}</SectionTitle>
      <div className="flex gap-1.5 -mx-3 px-3">
        {SUPPORTED_LANGUAGES.map(({ code, label_key }) => (
          <button
            key={code}
            type="button"
            onClick={() => i18n.changeLanguage(code)}
            className={cn(
              "flex-1 rounded-[8px] border px-2 py-1.5 text-[13px] font-medium transition-colors",
              i18n.resolvedLanguage === code
                ? "border-primary bg-primary/10 text-foreground"
                : "border-transparent bg-accent/50 text-muted-foreground hover:bg-accent",
            )}
          >
            {t(label_key)}
          </button>
        ))}
      </div>
    </div>
  );
}
