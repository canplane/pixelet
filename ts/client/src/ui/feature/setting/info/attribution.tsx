// @pxl/client/ui/feature/setting/info/attribution.tsx

import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import {
  Link,
  Prose,
  Section,
  SETTING_TEXT,
} from "@/ui/feature/setting/row.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { Licenses } from "./licenses.tsx";
import { WEIGHT } from "@/ui/shared/component/theme.ts";

/**
 * What the app is drawn with that is someone else's, by what it is: the map,
 * the assets, the font, the graphics, the system it runs on, and the packages
 * it is built on, its dependencies (licenses.tsx). Each provider
 * is named with its notice in its own words, not translated; a project known
 * by no name of its own is named as its repository, its owner and its name. Overture's is
 * worded as its own Explorer gives it (OvertureMaps/explore-site MapView.jsx
 * and TermsOfUse.jsx): one line, and its attribution guide, which carries
 * each source's terms. The land polygons' notice is the one in their
 * package's README. GEDTM30's carries the elements CC BY asks for. Arnis's is
 * the one in its NOTICE. NASA's imagery is credited as NASA asks, and the
 * star catalog as CDS asks of what it serves. GEDTM30's own inputs
 * (Copernicus DEM, AW3D30) bind GEDTM30's makers, who took them from their
 * licensors, and not those who take GEDTM30. What is CC0 asks for no notice
 * and is named as what the app is made with, and so is the font, whose
 * license the font file carries, linked to where its maker publishes it (Open
 * Font Library).
 */
export function AttributionSection() {
  const { t } = useTranslation();
  const odbl = (
    <Link href="https://opendatacommons.org/licenses/odbl/">
      Open Database License
    </Link>
  );
  const ccBy = (
    <Link href="https://creativecommons.org/licenses/by/4.0/">CC BY 4.0</Link>
  );
  const cc0 = (
    <Link href="https://creativecommons.org/publicdomain/zero/1.0/">
      CC0 1.0
    </Link>
  );
  return (
    <Section title={t("settings.attribution.section_title")}>
      <Prose>
        <View style={STYLE.groups}>
          <Group title={t("settings.attribution.map")}>
            <Provider name="Overture Maps" href="https://overturemaps.org">
              <Line>
                <Link href="https://www.openstreetmap.org/copyright">
                  © OpenStreetMap contributors
                </Link>
                , Overture Maps Foundation; modified. The datasets are subject
                to the licenses of third party data providers: see Overture's{" "}
                <Link href="https://docs.overturemaps.org/attribution/">
                  attribution and licensing guide
                </Link>
                .
              </Line>
            </Provider>
            <Provider
              name="OpenStreetMap Land Polygons"
              href="https://osmdata.openstreetmap.de/data/land-polygons.html"
            >
              <Line>
                This data is Copyright 2026{" "}
                <Link href="https://www.openstreetmap.org/copyright">
                  OpenStreetMap
                </Link>{" "}
                contributors. It is available under the {odbl} (ODbL).
              </Line>
            </Provider>
            <Provider
              name="GEDTM30"
              href="https://doi.org/10.5281/zenodo.18887460"
            >
              <Line>
                Modified GEDTM30 by Yufeng Ho and Tom Hengl (OpenGeoHub),
                licensed under {ccBy}
              </Line>
            </Provider>
            <Provider
              name="OpenStreetMap Carto"
              href="https://github.com/gravitystorm/openstreetmap-carto"
            >
              <Line>Place icons, {cc0}</Line>
            </Provider>
            <Provider
              name="louis-e/arnis"
              href="https://github.com/louis-e/arnis"
            >
              <Line>
                Bridges and tunnels. Copyright (c) 2022-2026 Louis Erbkamm
                (louis-e); modified; licensed under the{" "}
                <Link href="https://www.apache.org/licenses/LICENSE-2.0">
                  Apache License, Version 2.0
                </Link>
              </Line>
            </Provider>
          </Group>
          <Group title={t("settings.attribution.assets")}>
            <Provider
              name="NASA Earth Observatory"
              href="https://earthobservatory.nasa.gov/"
            >
              <Line>
                The Earth by day and night: Blue Marble Next Generation and
                Black Marble (Reto Stöckli; Miguel Román and the Black Marble
                team)
              </Line>
            </Provider>
            <Provider
              name="NASA Scientific Visualization Studio"
              href="https://svs.gsfc.nasa.gov/4720"
            >
              <Line>The Moon: CGI Moon Kit</Line>
            </Provider>
            <Provider
              name="Yale Bright Star Catalogue"
              href="https://cdsarc.cds.unistra.fr/viz-bin/cat/V/50"
            >
              <Line>The stars, as CDS, Strasbourg serves the catalogue</Line>
            </Provider>
            <Provider name="Quaternius" href="https://quaternius.com/">
              <Line>Clouds, {cc0}</Line>
            </Provider>
            <Provider name="Kenney" href="https://kenney.nl/">
              <Line>Characters and decor, {cc0}</Line>
            </Provider>
          </Group>
          <Group title={t("settings.attribution.font")}>
            <Provider
              name="u_DIN 1451 Mittelschrift"
              href="https://fontlibrary.org/en/font/u-din-1451-mittelschrift"
            >
              <Line>
                Copyright (c) 2016 usr_share, Copyright (c) 2009-2016 Peter
                Wiegel, licensed under the{" "}
                <Link href="https://scripts.sil.org/OFL">
                  SIL Open Font License, Version 1.1
                </Link>
              </Line>
            </Provider>
          </Group>
          <Group title={t("settings.attribution.graphics")}>
            <Provider
              name="N8python/n8ao"
              href="https://github.com/N8python/n8ao"
            >
              <Line>Ambient occlusion, {cc0}</Line>
            </Provider>
          </Group>
          <Group title={t("settings.attribution.system")}>
            <Provider
              name="evanw/buddy-malloc"
              href="https://github.com/evanw/buddy-malloc"
            >
              <Line>
                Memory allocator. Copyright 2018 Evan Wallace, licensed under
                the{" "}
                <Link href="https://opensource.org/license/mit">
                  MIT License
                </Link>
              </Line>
            </Provider>
          </Group>
          <Group title={t("settings.attribution.dependencies")}>
            <Licenses />
          </Group>
        </View>
      </Prose>
    </Section>
  );
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <View style={STYLE.group}>
      <Text style={STYLE.title}>{title}</Text>
      <View style={STYLE.providers}>{children}</View>
    </View>
  );
}

/** A provider, its name the link to its page, and its notice under the name;
 * a link the notice carries goes elsewhere, to a license or a guide. */
function Provider({
  name,
  href,
  children,
}: {
  name: string;
  href?: string;
  children: ReactNode;
}) {
  return (
    <View style={STYLE.provider}>
      <Text style={STYLE.name}>
        {href ? <Link href={href}>{name}</Link> : name}
      </Text>
      {children}
    </View>
  );
}

function Line({ children }: { children: ReactNode }) {
  return <Text style={SETTING_TEXT.small}>{children}</Text>;
}

const STYLE = StyleSheet.create({
  groups: { gap: 28 },
  group: { gap: 12 },
  title: {
    ...SETTING_TEXT.small,
    fontWeight: WEIGHT.semibold,
    color: LABEL.primary,
  },
  providers: { gap: 16 },
  provider: { gap: 4 },
  name: { ...SETTING_TEXT.small, fontWeight: WEIGHT.medium },
});
