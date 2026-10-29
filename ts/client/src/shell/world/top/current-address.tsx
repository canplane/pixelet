// @pxl/client/shell/world/top/current-address.tsx

import { StyleSheet } from "react-native";
import { useTranslation } from "react-i18next";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import {
  useDivisionLabel,
  usePivotCoordinates,
} from "@/shell/division/pivot.ts";
import { FADE } from "@/shell/ui/motion/transition.ts";
import { HaloText } from "./halo.tsx";
import {
  alpha,
  color,
  SECONDARY,
  textStyle,
  weight,
} from "@/shell/ui/theme.ts";

/**
 * Coordinates stand in where the pivot is in no place (sea, or not yet
 * arrived), and "Locating…" where there is no pivot yet.
 */
export function CurrentAddress() {
  const { t } = useTranslation();
  const label = useDivisionLabel();
  const position = usePivotCoordinates();

  const text = label ?? position;

  return (
    <Motion
      key={label ?? (position === null ? "locating" : "")}
      from={{ opacity: 0 }}
      to={{}}
      transition={FADE}
      style={styles.shrink}
    >
      <HaloText
        style={[
          styles.label,
          !label && styles.numbers,
          !text && styles.locating,
        ]}
      >
        {text ?? t("world.search.locating")}
      </HaloText>
    </Motion>
  );
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.body, fontWeight: weight.semibold, color: color.white },
  numbers: { fontVariant: ["tabular-nums"] },
  locating: { color: alpha(color.white, SECONDARY) },
});
