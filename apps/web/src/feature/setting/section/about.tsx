// @pxl/web/feature/setting/section/about.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Prose, Section, settingText } from "@/feature/setting/row.tsx";
import { BrandSymbol } from "@/shared/ui/brand-symbol.tsx";
import { textStyle } from "@/shared/ui/theme.ts";
import { APP_ENV, BUILD_NUMBER, BUILD_TIME, DEV_SOURCE } from "@/config/deploy.ts";
import { PUBLISHED_VERSION } from "@/config/published.spec.ts";
import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { LABEL } from "@platform/palette.ts";

const ENV_SHORT: Record<string, string> = { development: "dev", staging: "stg" };

/** `dev`, or the task that served it: `dev:local`, `dev:baked`. */
function envLabel(): string {
  const env = ENV_SHORT[APP_ENV] ?? APP_ENV;
  return DEV_SOURCE === "remote" ? env : `${env}:${DEV_SOURCE}`;
}

/** Each bake by the name it is published as, a line each, since each is baked on its own. */
function bakeLines(): string[] {
  return Object.entries(PUBLISHED_VERSION).map(([kind, name]) => `${kind}/${name}`);
}

/** The objects one has placed, kept on the device: their store's format. */
function objectLabel(): string {
  return STORAGE_KEYS[storage.OBJECTS].split(":").slice(1).join("/");
}

/** UTC: a stamp in the viewer's zone said different things on two phones. */
function buildLabel(): string {
  const d = new Date(BUILD_TIME);
  if (Number.isNaN(d.getTime())) return BUILD_TIME;
  const s = d.toISOString();
  return `${s.slice(0, 10).replaceAll("-", "")}T${s.slice(11, 16).replace(":", "")}Z`;
}

export function AboutSection() {
  const { t } = useTranslation();
  return (
    <Section title={t("settings.about.section_title")}>
      <Prose>
        <View style={styles.name}>
          <BrandSymbol size={20} color={LABEL.primary} />
          <Text style={styles.nameText}>pixelet</Text>
        </View>

        <Text style={settingText.small}>{t("settings.about.tagline")}</Text>

        <Text style={settingText.small}>© 2025 pixelet. All rights reserved.</Text>

        <View>
          <Text style={settingText.faint}>
            {BUILD_NUMBER}
            {APP_ENV !== "production" && ` (${envLabel()})`}
            {` · ${buildLabel()}`}
          </Text>
          {bakeLines().map((line) => <Text key={line} style={settingText.faint}>{line}</Text>)}
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
