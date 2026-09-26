// @pxl/web/app/unsupported.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { color, font } from "@/shared/ui/theme.ts";
import { useUnsupportedStore } from "./store/unsupported-store.ts";

/**
 * The end of the session, when the device turns out not to be able to run
 * this. Opaque and over everything: what is behind it is a map that will never
 * finish loading, and leaving it visible under a message would read as a page
 * still working. Over everything means over the toasts and the panels too,
 * which are the things on this page still claiming to be working.
 */
export function UnsupportedNotice() {
  const { t } = useTranslation();
  const reason = useUnsupportedStore((state) => state.reason);
  if (reason === null) return null;
  return (
    <View style={styles.cover}>
      <Text accessibilityRole="header" style={styles.title}>{t("unsupported.title")}</Text>
      <Text style={styles.body}>{t(`unsupported.${reason}`)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  cover: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    zIndex: 80,
    alignItems: "center",
    justifyContent: "center",
    gap: 12,
    paddingHorizontal: 32,
    backgroundColor: color.paper,
  },
  title: { fontFamily: font.family, fontSize: 18, fontWeight: "600", textAlign: "center", color: color.text },
  body: { maxWidth: 384, fontFamily: font.family, fontSize: 14, lineHeight: 23, textAlign: "center", color: color.textMuted },
});
