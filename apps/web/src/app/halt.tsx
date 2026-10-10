// @pxl/web/app/halt.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { color, textStyle } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

/** In place of the app, for good: where it cannot run, or what it needed did not come. `why` names
 * its words under `halt.`; the detail is the log's. */
export function Halt({ why }: { why: string }) {
  const { t } = useTranslation();
  return (
    <View style={styles.cover}>
      <Text accessibilityRole="header" style={styles.title}>{t(`halt.${why}.title`)}</Text>
      <Text style={styles.body}>{t(`halt.${why}.body`)}</Text>
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
