// @pxl/client/ui/feature/setting/info/about.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import {
  Link,
  Prose,
  Section,
  SETTING_TEXT,
} from "@/ui/feature/setting/row.tsx";
import { BrandSymbol } from "./brand-symbol.tsx";
import { TEXT_STYLE, WEIGHT } from "@/ui/shared/component/theme.ts";
import { BUILD_NUMBER, BUILD_TIME, TILE_SOURCE } from "@/build.ts";
import { format } from "@/abi/file.spec.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { useSourceVersions } from "@/ui/shared/watch.ts";

/** Shown in the settings in a dev build, because a phone has no URL bar to
 * tell which build is on screen. */
function devLabel(): string {
  return TILE_SOURCE === "delivery" ? "dev" : `dev:${TILE_SOURCE}`;
}

function bakeLines(source: Readonly<Record<string, string>> | null): string[] {
  return Object.entries(source ?? {}).map(
    ([kind, version]) => `${kind}/${version}`,
  );
}

function objectLabel(): string {
  return `objects · format ${format.OBJECT}`;
}

/** The build time is shown in UTC, because a time in the viewer's zone reads
 * differently on two phones. */
function buildLabel(): string {
  const d = new Date(BUILD_TIME);
  if (Number.isNaN(d.getTime())) return BUILD_TIME;
  const s = d.toISOString();
  return `${s.slice(0, 10).replaceAll("-", "")}T${s
    .slice(11, 16)
    .replace(":", "")}Z`;
}

export function AboutSection() {
  const { t } = useTranslation();
  const source = useSourceVersions();
  return (
    <Section title={t("settings.about.section_title")}>
      <Prose>
        <View style={STYLE.name}>
          <BrandSymbol size={20} color={LABEL.primary} />
          <Text style={STYLE.nameText}>
            <Text style={STYLE.project}>Project </Text>pixelet
          </Text>
        </View>

        <Text style={SETTING_TEXT.small}>{t("settings.about.tagline")}</Text>

        <Text style={SETTING_TEXT.small}>
          © 2025 <Link href="https://github.com/canplane">canplane</Link>. All
          rights reserved.
        </Text>

        <View>
          <Text style={SETTING_TEXT.faint}>
            {BUILD_NUMBER}
            {__DEV__ && ` (${devLabel()})`}
            {` · ${buildLabel()}`}
          </Text>
          {bakeLines(source).map((line) => (
            <Text key={line} style={SETTING_TEXT.faint}>
              {line}
            </Text>
          ))}
          <Text style={SETTING_TEXT.faint}>{objectLabel()}</Text>
        </View>
      </Prose>
    </Section>
  );
}

const STYLE = StyleSheet.create({
  name: { flexDirection: "row", alignItems: "center", gap: 8 },
  nameText: { ...TEXT_STYLE.title, color: LABEL.primary },
  /** What pixelet is, said under the name's own weight. */
  project: { fontWeight: WEIGHT.regular },
});
