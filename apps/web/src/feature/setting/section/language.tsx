// @pxl/web/feature/setting/section/language.tsx

import { Languages } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/util/cn.ts";
import { SectionTitle } from "@/feature/setting/row.tsx";

const SUPPORTED_LANGUAGES = [
  { code: "en", labelKey: "settings.language.english" },
  { code: "ko", labelKey: "settings.language.korean" },
] as const;

export function LanguageSection() {
  const { t, i18n } = useTranslation();

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Languages}>{t("settings.language.section_title")}</SectionTitle>
      <div className="flex gap-1.5 -mx-3 px-3">
        {SUPPORTED_LANGUAGES.map(({ code, labelKey }) => (
          <button
            key={code}
            type="button"
            onClick={() => i18n.changeLanguage(code)}
            className={cn(
              "flex-1 h-8 flex items-center justify-center rounded-full border px-2 text-[13px] font-medium transition-colors",
              i18n.resolvedLanguage === code ? "border-primary bg-primary/10 text-foreground" : "border-transparent bg-accent/50 text-muted-foreground hover:bg-accent",
            )}
          >
            {t(labelKey)}
          </button>
        ))}
      </div>
    </div>
  );
}
