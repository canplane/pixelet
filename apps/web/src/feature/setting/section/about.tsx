// @pxl/web/feature/setting/section/about.tsx

import { Box } from "lucide-react";
import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/feature/setting/row.tsx";
import { APP_ENV, BUILD_ID, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/config/deploy.ts";
import { PUBLISHED_VERSION } from "@/config/route.ts";
import { useKernel } from "@/kernel/shell.ts";

const envLabel: string = APP_ENV.charAt(0).toUpperCase() + APP_ENV.slice(1);

function buildLabel(): string {
  const d = new Date(BUILD_TIME);
  if (Number.isNaN(d.getTime())) return BUILD_TIME;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
}

export function AboutSection() {
  const { t } = useTranslation();
  const object_storage_version = useKernel((s) => s.object_storage_version);
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle>{t("settings.about.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[12px] text-muted-foreground">
        <div className="flex items-center gap-2 text-[15px] font-semibold text-foreground">
          <Box className="size-5" aria-hidden="true" />
          <span>pixelet</span>
        </div>

        <span>{t("settings.about.tagline")}</span>

        <span>© 2025 pixelet. All rights reserved.</span>

        <span className="text-muted-foreground/70">
          {APP_ENV !== "production" && `${envLabel} · `}
          {`${BUILD_NUMBER} · ${BUILD_ID} · ${buildLabel()}`}
          {` · ${PUBLISHED_TILES ? `Terrain ${PUBLISHED_VERSION.terrain}` : "Live terrain"}`}
          {` · ${PUBLISHED_TILES ? `POI ${PUBLISHED_VERSION.poi}` : "Live POI"}`}
          {` · Object v${object_storage_version}`}
        </span>
      </div>
    </div>
  );
}
