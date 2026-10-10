// @pxl/web/feature/world/ui/overlay/status-band.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { GroundStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { messageKey } from "@/locale/message-key.ts";
import { FADE } from "@/shared/ui/motion/transition.ts";
import { HaloText } from "@/shared/ui/halo.tsx";
import { color, textStyle, weight } from "@/shared/ui/theme.ts";

/** A state that holds while the world is on screen, said for as long as it holds; before, the load cover says it. */
export function StatusBand() {
  const { t } = useTranslation();
  const shown = useKernel((s) => s.focus.status === GroundStatus.Success);
  const stall = useKernel((s) => s.stall);
  const outside = useKernel((s) => s.focus.outside);
  // The world not coming outranks where the focus is.
  const key = !shown ? null : stall !== null ? messageKey(stall) : outside ? "status.outside" : null;
  if (key === null) return null;
  return (
    <Motion key={key} from={{ opacity: 0 }} to={{}} transition={FADE} style={styles.shrink}>
      <HaloText style={styles.words}>{t(key)}</HaloText>
    </Motion>
  );
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  words: { ...textStyle.footnote, fontWeight: weight.semibold, color: color.alarmGlyph, textAlign: "center" },
});
