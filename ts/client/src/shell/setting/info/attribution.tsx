// @pxl/client/shell/setting/info/attribution.tsx

import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Link, Prose, Section, settingText } from "@/shell/setting/row.tsx";
import { weight } from "@/shell/ui/theme.ts";

/**
 * License notices by the provider each comes from, not run through t().
 * Overture's are as its own Explorer gives them (OvertureMaps/explore-site
 * MapView.jsx, TermsOfUse.jsx): one line and its attribution guide, which
 * carries each source's terms. The land polygons' is their package's own
 * README; GEDTM30's is CC BY's elements; Arnis's is its NOTICE's. GEDTM30's
 * own inputs (Copernicus DEM, AW3D30) bind GEDTM30's makers, who took them
 * from their licensors, not those who take GEDTM30.
 */
export function AttributionSection() {
  const { t } = useTranslation();
  const odbl = <Link href="https://opendatacommons.org/licenses/odbl/">Open Database License</Link>;
  const ccBy = <Link href="https://creativecommons.org/licenses/by/4.0/">CC BY 4.0</Link>;
  return (
    <Section title={t("settings.attribution.section_title")}>
      <Prose>
        <View style={styles.providers}>
          <Provider name="Overture Maps">
            <Line>
              <Link href="https://www.openstreetmap.org/copyright">© OpenStreetMap contributors</Link>,{" "}
              <Link href="https://overturemaps.org">Overture Maps Foundation</Link>; modified. The datasets are subject to the licenses of third party data providers: see Overture's{" "}
              <Link href="https://docs.overturemaps.org/attribution/">attribution and licensing guide</Link>.
            </Line>
          </Provider>
          <Provider name="OpenStreetMap Land Polygons">
            <Line>
              This data is Copyright 2026 <Link href="https://www.openstreetmap.org/copyright">OpenStreetMap</Link> contributors. It is available under the {odbl} (ODbL).
            </Line>
          </Provider>
          <Provider name="GEDTM30">
            <Line>
              Modified <Link href="https://doi.org/10.5281/zenodo.18887460">GEDTM30</Link> by Yufeng Ho and Tom Hengl (OpenGeoHub), licensed under {ccBy}
            </Line>
          </Provider>
          <Provider name="Arnis">
            <Line>
              Bridges and tunnels: modified from <Link href="https://github.com/louis-e/arnis">Arnis</Link>, Copyright (c) 2022-2026 Louis Erbkamm (louis-e), licensed under the{" "}
              <Link href="https://www.apache.org/licenses/LICENSE-2.0">Apache License, Version 2.0</Link>
            </Line>
          </Provider>
        </View>
      </Prose>
    </Section>
  );
}

/** A provider's name over its notices, apart from the next provider's. */
function Provider({ name, children }: { name: string; children: ReactNode }) {
  return (
    <View style={styles.provider}>
      <Text style={styles.name}>{name}</Text>
      {children}
    </View>
  );
}

function Line({ children }: { children: ReactNode }) {
  return <Text style={settingText.small}>{children}</Text>;
}

const styles = StyleSheet.create({
  providers: { gap: 16 },
  provider: { gap: 4 },
  name: { ...settingText.small, fontWeight: weight.medium },
});
