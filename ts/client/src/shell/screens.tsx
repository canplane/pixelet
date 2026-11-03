// @pxl/client/shell/screens.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { useKernel } from "@/shell/store/watch.ts";
import { color, textStyle } from "@/shell/ui/theme.ts";
import { LABEL } from "@/shell/ui/palette.ts";
import { ProgressBar } from "./progress-bar.tsx";

/** Over the world while it starts: the bar, and under it what it waits on. `of`
 * is the locale key of that line. */
export function Progress({ of }: { of: string }) {
  const { t } = useTranslation();
  const made = useKernel((s) => s.renderer_progress);
  return (
    <View style={styles.cover}>
      <ProgressBar made={made} />
      <Text style={styles.line}>{t(of)}</Text>
    </View>
  );
}

/** In place of the app before it ran, for good: where it cannot run, or what it
 * needed will not come. `of` is the locale key its title and body are under; the
 * detail is the log's. */
export function Failed({ of }: { of: string }) {
  const { t } = useTranslation();
  return (
    <View style={styles.cover}>
      <Text accessibilityRole="header" style={styles.title}>
        {t(`${of}.title`)}
      </Text>
      <Text style={styles.body}>{t(`${of}.body`)}</Text>
    </View>
  );
}

/** In place of the app once what ran has ended, as a blue screen says it: the
 * stop code, where there is one, is what a screenshot sent back needs. */
export function Panic({ of, code }: { of: string; code: string | null }) {
  const { t } = useTranslation();
  return (
    <View style={[styles.cover, styles.panic]}>
      <View style={styles.column}>
        <Text style={styles.face}>:(</Text>
        <Text accessibilityRole="header" style={styles.panicTitle}>
          {t(`${of}.title`)}
        </Text>
        <Text style={styles.panicBody}>{t(`${of}.body`)}</Text>
        {code !== null && (
          <Text selectable style={styles.code}>
            {t("panic.stop_code", { code })}
          </Text>
        )}
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
  line: { ...textStyle.footnote, textAlign: "center", color: LABEL.secondary },
  title: { ...textStyle.title2, textAlign: "center", color: LABEL.primary },
  body: {
    maxWidth: 384,
    ...textStyle.body,
    textAlign: "center",
    color: LABEL.secondary,
  },
  panic: { alignItems: "stretch", backgroundColor: color.panic },
  column: { width: "100%", maxWidth: 480, alignSelf: "center", gap: 16 },
  face: {
    fontFamily: textStyle.title2.fontFamily,
    fontSize: 96,
    lineHeight: 112,
    color: color.white,
  },
  panicTitle: { ...textStyle.title2, color: color.white },
  panicBody: { ...textStyle.body, color: color.white },
  code: { ...textStyle.footnote, marginTop: 16, color: color.white },
});
