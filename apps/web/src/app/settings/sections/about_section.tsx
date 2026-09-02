// @pxl/web/app/settings/sections/about_section.tsx

import { Box } from "lucide-react";
import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";
import { APP_ENV, BUILD_TIME, HAS_STATIC_TILES, TILE_BASE_URL } from "@/lib/env/config.ts";
import { OBJECT_STORAGE_VERSION } from "@/modules/world/objects/store.ts";

const tile_version: string = (TILE_BASE_URL.split("/").filter(Boolean).pop() ?? "").replace(/^v/, "");

const env_label: string = APP_ENV.charAt(0).toUpperCase() + APP_ENV.slice(1);

function build_label(): string {
  const d = new Date(BUILD_TIME);
  if (Number.isNaN(d.getTime())) return BUILD_TIME;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
}

export function AboutSection() {
  const { t } = useTranslation();
  const contact_url = "https://github.com/canplane";
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle>{t("settings.about.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[12px] text-muted-foreground">
        <div className="flex items-center gap-2 text-[15px] font-semibold text-foreground">
          <Box className="size-5" aria-hidden="true" />
          <span>pixelet</span>
        </div>

        <span>{t("settings.about.tagline")}</span>

        <span>
          © 2025{" "}
          <a href={contact_url} target="_blank" rel="noreferrer" className="underline hover:text-foreground">
            canplane
          </a>
          . All rights reserved.
        </span>

        <span className="text-muted-foreground/70">
          {APP_ENV !== "production" && `${env_label} · `}
          {build_label()}
          {` · ${HAS_STATIC_TILES ? `Tile v${tile_version}` : "Live tile"}`}
          {` · Object v${OBJECT_STORAGE_VERSION}`}
        </span>
      </div>
    </div>
  );
}
