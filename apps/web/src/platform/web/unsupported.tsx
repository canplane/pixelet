// @pxl/web/platform/web/unsupported.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { color, textStyle } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";
import type { Missing } from "@/platform/web/webgpu.ts";

/** Shown in place of the app before anything starts. */
export function Unsupported({ reason }: { reason: Missing }) {
  const { t } = useTranslation();
  return (
    <View style={styles.cover}>
      <Text accessibilityRole="header" style={styles.title}>{t("unsupported.title")}</Text>
      <Text style={styles.body}>{t(`unsupported.${reason}`)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  cover: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: 12,
    paddingHorizontal: 32,
    backgroundColor: color.ground,
  },
  title: { ...textStyle.title2, textAlign: "center", color: LABEL.primary },
  body: { maxWidth: 384, ...textStyle.body, textAlign: "center", color: LABEL.secondary },
});
