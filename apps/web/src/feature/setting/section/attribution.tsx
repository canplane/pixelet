// @pxl/web/feature/setting/section/attribution.tsx

import { useTranslation } from "react-i18next";
import { Text } from "react-native";

import { Link, Prose, Section, settingText } from "@/feature/setting/row.tsx";

/**
 * License notices, not run through t(): their wording is prescribed. OSM's
 * guidelines require naming ODbL; CC BY 4.0 asks for creator, title, license
 * link and a note of changes; the Copernicus "produced using" line is
 * verbatim and compulsory. JAXA's required wording is still unconfirmed.
 */
export function AttributionSection() {
  const { t } = useTranslation();
  return (
    <Section title={t("settings.attribution.section_title")}>
      <Prose>
        <Text style={settingText.small}>
          © <Link href="https://www.openstreetmap.org/copyright">OpenStreetMap</Link> contributors, available under the{" "}
          <Link href="https://opendatacommons.org/licenses/odbl/">Open Database License</Link>
        </Text>
        <Text style={settingText.small}>
          Elevation: modified <Link href="https://doi.org/10.5281/zenodo.15689805">GEDTM30</Link> by OpenGeoHub, licensed under{" "}
          <Link href="https://creativecommons.org/licenses/by/4.0/">CC BY 4.0</Link>
        </Text>
        <Text style={settingText.small}>
          produced using Copernicus WorldDEM © DLR e.V. 2010–2014 and © Airbus Defense and Space GmbH 2014–2018 provided under COPERNICUS by the European Union and ESA; all rights reserved
        </Text>
      </Prose>
    </Section>
  );
}
