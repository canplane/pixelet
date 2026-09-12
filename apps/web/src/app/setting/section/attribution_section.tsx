// @pxl/web/app/setting/section/attribution_section.tsx

import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/setting/setting_row.tsx";

/**
 * Source notices. Hardcoded rather than run through t(): these are licence
 * text, required in the same form regardless of the UI language.
 *
 * What each line is doing, and which part of it is actually compulsory —
 * because the previous version of this file had it backwards, carrying an
 * academic courtesy in full while omitting a notice whose wording a licence
 * prescribes.
 *
 * **Map data.** Overture's Base/Buildings/Transportation/Divisions are ODbL
 * and substantially OSM-sourced, so the obligation runs to OSM's
 * contributors; Places is CDLA-Permissive-2.0, which asks only that the
 * licence text travel with the data. The OSMF Attribution Guidelines require
 * the credit to make clear the data is under ODbL, so the licence is named
 * outright and the word links to openstreetmap.org/copyright — the same form
 * Overture displays for its own ODbL themes.
 *
 * Overture Maps Foundation itself is deliberately not credited. Its
 * attribution page offers a citation a user "may choose to add"; nothing in
 * either licence compels it, and a notice section earns its length by being
 * only what is owed.
 *
 * **Elevation.** GEDTM30 is CC BY 4.0, which asks for the creator, the title,
 * a link to the licence, and an indication that changes were made — the
 * pipeline crops and reprojects, so it changed. That is the whole of it. The
 * author list and journal reference that used to sit here are an academic
 * courtesy, not part of what the licence asks, and they were long enough to
 * crowd out the line below, which is not optional. The link is the DOI of the
 * distribution this pipeline actually consumes
 * (apps/api/src/bake/elevation/metadata.json), so the notice, the README and the ingest
 * script all name one source instead of three mirrors of it.
 *
 * **Copernicus.** This one is compulsory and its wording is dictated. GEDTM30
 * is a fusion whose inputs include Copernicus WorldDEM, and the Copernicus
 * DEM licence binds anyone communicating adapted data to the "produced using"
 * form of its notice, reproduced verbatim below.
 *
 * GEDTM30 also fuses JAXA's ALOS World 3D-30m. JAXA's own terms page was not
 * readable at the time of writing, so no claim is made about its exact
 * required wording rather than inventing one — worth closing.
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
          contributors, available under the{" "}
          <a
            href="https://opendatacommons.org/licenses/odbl/"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            Open Database License
          </a>
        </span>
        <span>
          Elevation: modified{" "}
          <a
            href="https://doi.org/10.5281/zenodo.15689805"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            GEDTM30
          </a>{" "}
          by OpenGeoHub, licensed under{" "}
          <a
            href="https://creativecommons.org/licenses/by/4.0/"
            target="_blank"
            rel="noreferrer"
            className="underline hover:text-foreground"
          >
            CC BY 4.0
          </a>
        </span>
        <span>
          produced using Copernicus WorldDEM © DLR e.V. 2010–2014 and © Airbus Defence and Space GmbH 2014–2018 provided under COPERNICUS by the European Union and ESA; all rights reserved
        </span>
      </div>
    </div>
  );
}
