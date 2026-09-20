// @pxl/web/feature/setting/section/about.tsx

import { Box } from "lucide-react";
import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/feature/setting/row.tsx";
import { APP_ENV, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/config/deploy.ts";
import { PUBLISHED_VERSION } from "@/config/route.ts";
import { useKernel } from "@/kernel/shell.ts";

/** The environments worth abbreviating; anything else is shown as it is set. */
const ENV_SHORT: Record<string, string> = { development: "dev", staging: "stg" };

/** What the map is made of, in the order it is built: the ground and what
 * stands on it, then what the places on it are called. A bake reads as the
 * prefix it is served from, `<kind>/<version>`, which is a path anyone can
 * put in a URL; the object store's version is not a path and is not spelled
 * as one. Where the tiles are a live API instead, that is a property of the
 * deployment rather than of any one kind, so it is said once. */
function dataLabel(object_storage_version: number): string {
  const object = `object-v${object_storage_version}`;
  if (!PUBLISHED_TILES) return `live tiles · ${object}`;
  const { terrain, poi, geocode } = PUBLISHED_VERSION;
  return `terrain/${terrain} · ${object} · poi/${poi} · geocode/${geocode}`;
}

/** The build time in ISO 8601's basic format, UTC: one build reads as one
 * string wherever it is read, which a stamp meant to be quoted back has to.
 * It was the viewer's own timezone before, and a stamp with no zone in it
 * said different things on two phones. */
function buildLabel(): string {
  const d = new Date(BUILD_TIME);
  if (Number.isNaN(d.getTime())) return BUILD_TIME;
  const s = d.toISOString();
  return `${s.slice(0, 10).replaceAll("-", "")}T${s.slice(11, 16).replace(":", "")}Z`;
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

        <div className="flex flex-col text-muted-foreground/70">
          <span>
            {BUILD_NUMBER}
            {APP_ENV !== "production" && ` (${ENV_SHORT[APP_ENV] ?? APP_ENV})`}
            {` · ${buildLabel()}`}
          </span>
          <span>{dataLabel(object_storage_version)}</span>
        </div>
      </div>
    </div>
  );
}
