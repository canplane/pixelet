// @pxl/web/app/settings/sections/disclaimer_section.tsx

import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

export function DisclaimerSection() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle>{t("settings.disclaimer.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[12px] text-muted-foreground">
        <span>{t("settings.disclaimer.demo_notice")}</span>

        <span>{t("settings.disclaimer.coverage_notice")}</span>

        <span>{t("settings.disclaimer.storage_notice")}</span>

        <span>{t("settings.disclaimer.location_notice")}</span>
      </div>
    </div>
  );
}
