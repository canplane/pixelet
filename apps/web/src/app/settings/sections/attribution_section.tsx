// @pxl/web/app/settings/sections/attribution_section.tsx

import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

// OSM/GEDTM30/Overture attribution text — hardcoded rather than run through t(): required (OSM, GEDTM30's CC-BY 4.0) or shown as a courtesy (Overture's Places theme, CDLA-Permissive-2.0 imposes no attribution requirement) verbatim regardless of UI language.
export function AttributionSection() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle>{t("settings.attribution.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[12px] text-muted-foreground">
        <span>
          ©{" "}
          <a
            href="https://www.openstreetmap.org/copyright"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            OpenStreetMap
          </a>{" "}
          contributors
        </span>
        <span>
          Contains modified{" "}
          <a
            href="https://doi.org/10.5281/zenodo.15689805"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            GEDTM30
          </a>{" "}
          data. Ho Y, Grohmann CH, Lindsay J, Reuter HI, Parente L, Witjes M, Hengl T (2025). GEDTM30: global ensemble digital terrain model at 30 m and derived multiscale terrain variables. PeerJ
          13:e19673, CC-BY 4.0
        </span>
        <span>
          Places data ©{" "}
          <a
            href="https://overturemaps.org/"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            Overture Maps Foundation
          </a>{" "}
          and contributors
        </span>
      </div>
    </div>
  );
}
