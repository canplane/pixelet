// @pxl/web/app/settings/sections/attribution_section.tsx

import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

/**
 * Other people's terms, reproduced because they require it: the OSM and
 * Copernicus data-source attribution. Hardcoded rather than run through
 * t() — required verbatim regardless of UI language.
 *
 * "Attribution", not "License". A LICENSE says what terms *this* work is
 * offered under, which is a claim we make; everything here is an
 * acknowledgement of someone else's, which every platform names differently
 * but never that (Android: "Open source licenses", iOS: "Acknowledgements",
 * generally "Third-Party Notices"). This section was called License while
 * it still held "© 2025 canplane. All rights reserved." — and that line was
 * the problem too, since sitting above the others it read as if it covered
 * them, with "all rights reserved" directly over ODbL and CC0 sources
 * sending two signals at once. The claim moved to about_section.tsx next to
 * the name it belongs to, and this took the name that matches what is left.
 *
 * Nothing else belongs here. Everything this list once carried besides
 * attribution now lives in disclaimer_section.tsx (what to expect) or
 * about_section.tsx (which build) — see the former for why mixing the three
 * hurt all of them. The texture pack is the one credit deliberately not
 * repeated here: CC0 obliges nothing, and the disclaimer's own texture line
 * already names and links it.
 */
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
      </div>
    </div>
  );
}
