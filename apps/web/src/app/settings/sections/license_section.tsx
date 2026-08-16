// @pxl/web/app/settings/sections/license_section.tsx

import { Box, ScrollText } from "lucide-react";
import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

/** Required OSM/Copernicus data-source attribution, plus this project's own copyright notice. Legal/attribution text stays hardcoded (not run through t()) — it's required verbatim regardless of UI language, same as the OSM/Copernicus lines already were before this file had any translation at all. */
export function LicenseSection() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={ScrollText}>{t("settings.license.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[11px] text-muted-foreground">
        <div className="flex items-center gap-1.5 text-xs font-medium text-foreground">
          <Box className="h-3.5 w-3.5" aria-hidden="true" />
          <span>pixelet</span>
        </div>
        <span>
          © 2025{" "}
          <a href="https://github.com/canplane" target="_blank" rel="noreferrer" className="underline hover:text-foreground">
            canplane
          </a>
          . All rights reserved.
        </span>
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
        {/* Unlike the attribution lines above, this one IS translated: it's a
            disclosure to the person using the app, not a notice a licence
            requires verbatim. See use_reverse_geocode.ts for the request it
            describes and the rounding it cites. */}
        <span>{t("settings.license.geocoding_notice")}</span>
      </div>
    </div>
  );
}
