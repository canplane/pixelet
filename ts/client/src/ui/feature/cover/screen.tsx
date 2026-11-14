// @pxl/client/ui/feature/cover/screen.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { COLOR, TEXT_STYLE } from "@/ui/shared/component/theme.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";
import type { Cover } from "./cover.ts";
import { ProgressBar } from "./progress-bar.tsx";

/** The bar stands at the middle of the screen, and the line hangs under it, so
 * a line that wraps to more rows moves nothing. */
function Progress({ of, share }: { of: string; share: number }) {
  const { t } = useTranslation();
  return (
    <View style={styles.cover}>
      <View style={styles.bar}>
        <ProgressBar share={share} />
        <Text style={styles.line}>{t(of)}</Text>
      </View>
    </View>
  );
}

/** Shown for good in place of an app that never ran, because it cannot run
 * here or because what it needed will not come. The details go to the log, not
 * to this screen. */
function Failed({ of }: { of: string }) {
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

/** Shown in place of the app once what ran has ended, in the manner of a blue
 * screen. The stop code, where there is one, is what a screenshot sent back
 * needs. */
function Panic({ of, code }: { of: string; code: string | null }) {
  const { t } = useTranslation();
  return (
    <View style={[styles.cover, styles.panic]}>
      <View style={styles.column}>
        <Text style={styles.face}>:(</Text>
        <Text accessibilityRole="header" style={styles.panic_title}>
          {t(`${of}.title`)}
        </Text>
        <Text style={styles.panic_body}>{t(`${of}.body`)}</Text>
        {code !== null && (
          <Text selectable style={styles.code}>
            {t("panic.stop_code", { code })}
          </Text>
        )}
      </View>
    </View>
  );
}

export function Screen({ cover }: { cover: Cover }) {
  switch (cover.kind) {
    case "progress":
      return <Progress of={cover.of} share={cover.share} />;
    case "failed":
      return <Failed of={cover.of} />;
    case "panic":
      return <Panic of={cover.of} code={cover.code} />;
  }
}

const styles = StyleSheet.create({
  cover: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: 12,
    paddingHorizontal: 32,
    backgroundColor: COLOR.ground,
  },
  bar: { alignSelf: "stretch", alignItems: "center" },
  line: {
    position: "absolute",
    top: "100%",
    left: 0,
    right: 0,
    marginTop: 12,
    ...TEXT_STYLE.footnote,
    textAlign: "center",
    color: LABEL.secondary,
  },
  title: { ...TEXT_STYLE.title2, textAlign: "center", color: LABEL.primary },
  body: {
    maxWidth: 384,
    ...TEXT_STYLE.body,
    textAlign: "center",
    color: LABEL.secondary,
  },
  panic: { alignItems: "stretch", backgroundColor: COLOR.panic },
  column: { width: "100%", maxWidth: 480, alignSelf: "center", gap: 16 },
  face: {
    fontSize: 96,
    lineHeight: 112,
    color: COLOR.white,
  },
  panic_title: { ...TEXT_STYLE.title2, color: COLOR.white },
  panic_body: { ...TEXT_STYLE.body, color: COLOR.white },
  code: { ...TEXT_STYLE.footnote, marginTop: 16, color: COLOR.white },
});
