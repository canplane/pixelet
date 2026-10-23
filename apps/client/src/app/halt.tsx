// @pxl/client/app/halt.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { color, textStyle } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

/** In place of the app, for good: where it cannot run, or what it needed did not come. `of` is the
 * locale key its title and body are under; the detail is the log's. */
export function Halt({ of }: { of: string }) {
  const { t } = useTranslation();
  return (
    <View style={styles.cover}>
      <Text accessibilityRole="header" style={styles.title}>{t(`${of}.title`)}</Text>
      <Text style={styles.body}>{t(`${of}.body`)}</Text>
    </View>
  );
}

/** In place of the app once what ran has ended, as a blue screen says it, apart from a place it
 * cannot run: the stop code, where there is one, is what a screenshot sent back needs. */
export function BlueScreen({ of, code }: { of: string; code: string | null }) {
  const { t } = useTranslation();
  return (
    <View style={[styles.cover, styles.panic]}>
      <View style={styles.column}>
        <Text style={styles.face}>:(</Text>
        <Text accessibilityRole="header" style={styles.panicTitle}>{t(`${of}.title`)}</Text>
        <Text style={styles.panicBody}>{t(`${of}.body`)}</Text>
        {code !== null && <Text selectable style={styles.code}>{t("halt.stop_code", { code })}</Text>}
      </View>
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
  panic: { alignItems: "stretch", backgroundColor: color.panic },
  column: { width: "100%", maxWidth: 480, alignSelf: "center", gap: 16 },
  face: { fontFamily: textStyle.title2.fontFamily, fontSize: 96, lineHeight: 112, color: color.white },
  panicTitle: { ...textStyle.title2, color: color.white },
  panicBody: { ...textStyle.body, color: color.white },
  code: { ...textStyle.footnote, marginTop: 16, color: color.white },
});
