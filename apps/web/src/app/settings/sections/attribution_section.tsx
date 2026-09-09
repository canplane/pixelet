// @pxl/web/app/settings/sections/attribution_section.tsx

import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

// OSM/Copernicus/Overture attribution text — hardcoded rather than run through t(): required (OSM, Copernicus) or shown as a courtesy (Overture's Places theme, CDLA-Permissive-2.0 imposes no attribution requirement) verbatim regardless of UI language.
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
          produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under{" "}
          <a
            href="https://www.copernicus.eu/en"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            COPERNICUS
          </a>{" "}
          by the European Union and ESA; all rights reserved
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
