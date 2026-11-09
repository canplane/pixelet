// @pxl/client/ui/feature/setting/info/about.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Prose, Section, settingText } from "@/ui/feature/setting/row.tsx";
import { BrandSymbol } from "./brand-symbol.tsx";
import { textStyle } from "@/ui/shared/component/theme.ts";
import { APP_ENV, BUILD_NUMBER, BUILD_TIME, DEV_SOURCE } from "@/build.ts";
import { format } from "@/abi/file.spec.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { useSourceVersions } from "@/ui/shared/watch.ts";

function envLabel(): string {
  return DEV_SOURCE === "delivery" ? APP_ENV : `${APP_ENV}:${DEV_SOURCE}`;
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
        <View style={styles.name}>
          <BrandSymbol size={20} color={LABEL.primary} />
          <Text style={styles.nameText}>pixelet</Text>
        </View>

        <Text style={settingText.small}>{t("settings.about.tagline")}</Text>

        <Text style={settingText.small}>
          © 2025 pixelet. All rights reserved.
        </Text>

        <View>
          <Text style={settingText.faint}>
            {BUILD_NUMBER}
            {APP_ENV !== "prod" && ` (${envLabel()})`}
            {` · ${buildLabel()}`}
          </Text>
          {bakeLines(source).map((line) => (
            <Text key={line} style={settingText.faint}>
              {line}
            </Text>
          ))}
          <Text style={settingText.faint}>{objectLabel()}</Text>
        </View>
      </Prose>
    </Section>
  );
}

const styles = StyleSheet.create({
  name: { flexDirection: "row", alignItems: "center", gap: 8 },
  nameText: { ...textStyle.title, color: LABEL.primary },
});
