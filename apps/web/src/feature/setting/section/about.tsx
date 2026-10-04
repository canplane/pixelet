// @pxl/web/feature/setting/section/about.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { SectionTitle, settingText } from "@/feature/setting/row.tsx";
import { BrandSymbol } from "@/shared/ui/brand-symbol.tsx";
import { textStyle } from "@/shared/ui/theme.ts";
import { APP_ENV, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/config/deploy.ts";
import { PUBLISHED_VERSION } from "@/config/published.spec.ts";
import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { LABEL } from "@platform/palette.ts";

const ENV_SHORT: Record<string, string> = { development: "dev", staging: "stg" };

function dataLabel(): string {
  const object = STORAGE_KEYS[storage.OBJECTS].split(":").slice(1).join("/");
  if (!PUBLISHED_TILES) return `live tiles · ${object}`;
  const { terrain, poi, geocode } = PUBLISHED_VERSION;
  return `terrain/${terrain} · ${object} · poi/${poi} · geocode/${geocode}`;
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
    <View style={styles.section}>
      <SectionTitle>{t("settings.about.section_title")}</SectionTitle>
      <View style={styles.body}>
        <View style={styles.name}>
          <BrandSymbol size={20} color={LABEL.primary} />
          <Text style={styles.nameText}>pixelet</Text>
        </View>

        <Text style={settingText.small}>{t("settings.about.tagline")}</Text>

        <Text style={settingText.small}>© 2025 pixelet. All rights reserved.</Text>

        <View>
          <Text style={settingText.faint}>
            {BUILD_NUMBER}
            {APP_ENV !== "production" && ` (${ENV_SHORT[APP_ENV] ?? APP_ENV})`}
            {` · ${buildLabel()}`}
          </Text>
          <Text style={settingText.faint}>{dataLabel()}</Text>
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  section: { gap: 4 },
  body: { gap: 8, paddingVertical: 4 },
  name: { flexDirection: "row", alignItems: "center", gap: 8 },
  nameText: { ...textStyle.title, color: LABEL.primary },
});
