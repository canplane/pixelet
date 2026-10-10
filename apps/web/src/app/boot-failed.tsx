// @pxl/web/app/boot-failed.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { STATUS } from "@platform/palette.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { chrome, color, textStyle } from "@/shared/ui/theme.ts";

/** A kernel that never started has no world and no toasts to say it, so it is said here. */
export function BootFailed() {
  const { t } = useTranslation();
  return (
    <View style={styles.cover} pointerEvents="none">
      <View style={[styles.disc, { backgroundColor: STATUS.failed }]}>
        <Glyph glyph="xmark" size={14} weight="black" color={color.white} />
      </View>
      <Text style={styles.words}>{t("boot.failed")}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  cover: { position: "absolute", top: 0, right: 0, bottom: 0, left: 0, alignItems: "center", justifyContent: "center", gap: 12, paddingHorizontal: 32 },
  disc: { width: chrome.glyph + 4, height: chrome.glyph + 4, borderRadius: (chrome.glyph + 4) / 2, alignItems: "center", justifyContent: "center" },
  words: { maxWidth: 384, ...textStyle.body, textAlign: "center", color: color.white },
});
