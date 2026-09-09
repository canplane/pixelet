// @pxl/web/app/settings/sections/attribution_section.tsx

import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

// OSM/GEDTM30 attribution text — hardcoded rather than run through t(), both
// legally required verbatim regardless of UI language: OSM's ODbL, and
// GEDTM30's CC-BY 4.0. No separate Overture Maps Foundation line: Places'
// CDLA-Permissive-2.0 has no attribution clause at all (confirmed against
// the license text — it only requires sharing the license text alongside
// the data, not crediting a source), and Buildings/Transportation/Base/
// Divisions carry ODbL only because they're substantially OSM-sourced —
// that obligation runs to OSM's contributors, already covered by the line
// below, not to Overture as the redistributor. Overture's own attribution
// page (docs.overturemaps.org/attribution) recommends co-crediting itself
// for those themes as its own best practice, but that's a suggestion, not
// something either license mandates — kept at the legally-required minimum.
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
      </div>
    </div>
  );
}
