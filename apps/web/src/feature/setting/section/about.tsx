// @pxl/web/feature/setting/section/about.tsx

import { Box } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { SectionTitle, settingText } from "@/feature/setting/row.tsx";
import { color, font } from "@/shared/ui/theme.ts";
import { APP_ENV, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/config/deploy.ts";
import { PUBLISHED_VERSION } from "@/config/route.ts";
import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";

/** The environments worth abbreviating; anything else is shown as it is set. */
const ENV_SHORT: Record<string, string> = { development: "dev", staging: "stg" };

/** What the map is made of, in the order it is built: the ground and what
 * stands on it, then what the places on it are called. Each reads as
 * `<what>/<version>` — for a bake that is the prefix it is served from, a
 * path anyone can put in a URL, and the object store's version is written
 * the same way so the line has one form rather than two. Where the tiles are
 * a live API instead, that is a property of the deployment rather than of
 * any one kind, so it is said once. */
function dataLabel(): string {
  const object = STORAGE_KEYS[storage.OBJECTS].split(":").slice(1).join("/");
  if (!PUBLISHED_TILES) return `live tiles · ${object}`;
  const { terrain, poi, geocode } = PUBLISHED_VERSION;
  return `terrain/${terrain} · ${object} · poi/${poi} · geocode/${geocode}`;
}

/** The build time in ISO 8601's basic format, UTC: one build reads as one
 * string wherever it is read, which a stamp meant to be quoted back has to.
 * It was the viewer's own timezone before, and a stamp with no zone in it
 * said different things on two phones. */
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
          <Box size={20} color={color.text} />
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
  nameText: { fontFamily: font.family, fontSize: 15, fontWeight: "600", color: color.text },
});
