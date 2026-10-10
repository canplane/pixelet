// @pxl/web/app/load-cover.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Spinner } from "@platform/icon.tsx";
import { STATUS } from "@platform/palette.ts";
import { GroundStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { HostTrouble, useTrouble } from "@/kernel/trouble.ts";
import { messageKey } from "@/locale/message-key.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { chrome, color, textStyle } from "@/shared/ui/theme.ts";

const HOST: Record<Exclude<HostTrouble, HostTrouble.None>, string> = {
  [HostTrouble.BootFailed]: "load.boot_failed",
  [HostTrouble.NoGpu]: "unsupported.gpu",
};

/** Over a screen that shows no world yet, black: that it is coming, or why it cannot. */
export function LoadCover() {
  const { t } = useTranslation();
  const trouble = useTrouble();
  const shown = useKernel((s) => s.focus.status === GroundStatus.Success);
  const stall = useKernel((s) => s.stall);
  if (shown && trouble === HostTrouble.None) return null;
  // The host's trouble ends the start; the kernel's is waited out, its reads retried.
  const failed = trouble !== HostTrouble.None;
  const words = failed ? t(HOST[trouble]) : stall !== null ? t(messageKey(stall)) : t("load.loading");
  return (
    <View style={styles.cover} pointerEvents="none">
      {failed
        ? (
          <View style={[styles.disc, { backgroundColor: STATUS.failed }]}>
            <Glyph glyph="xmark" size={14} weight="black" color={color.white} />
          </View>
        )
        : <Spinner size={chrome.glyph} color={color.white} />}
      <Text style={styles.words}>{words}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  cover: { position: "absolute", top: 0, right: 0, bottom: 0, left: 0, alignItems: "center", justifyContent: "center", gap: 12, paddingHorizontal: 32 },
  disc: { width: chrome.glyph + 4, height: chrome.glyph + 4, borderRadius: (chrome.glyph + 4) / 2, alignItems: "center", justifyContent: "center" },
  words: { maxWidth: 384, ...textStyle.body, textAlign: "center", color: color.white },
});
