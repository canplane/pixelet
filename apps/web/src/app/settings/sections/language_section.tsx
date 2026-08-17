// @pxl/web/app/settings/sections/language_section.tsx

import { Languages } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/utils/cn.ts";
import { SectionTitle } from "@/app/settings/setting_row.tsx";

// Korean first. This is the order the buttons render in, and the author and
// the demo's audience both read Korean — English is the fallback here, not
// the default. (i18n.ts still resolves the initial language from the
// browser; this only decides which button sits on the left.)
const SUPPORTED_LANGUAGES = [
  { code: "ko", label_key: "settings.language.korean" },
  { code: "en", label_key: "settings.language.english" },
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
              // h-8 (32), measured off Instagram's own Follow/Message pair —
              // the same thing this is, a row of equal-width action buttons
              // filling the content width. Theirs: 32 tall, 16 side margin,
              // 5 between the two, ~8 radius, label around 13 — which is
              // what the text is set at too, rather than the 15 the setting
              // rows use: those are body copy, this is a button label, and
              // 15 in a 32-tall button left almost no room around it.
              // py-2.5 here came out near 42, and a first pass at 38 was
              // still six over.
              // Fixed height rather than padding-derived so the pair stays
              // level regardless of what the two labels render at.
              "flex-1 h-8 flex items-center justify-center rounded-[8px] border px-2 text-[13px] font-medium transition-colors",
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
