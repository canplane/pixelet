// @pxl/client/ui/feature/world/top/current-address.tsx

import { StyleSheet } from "react-native";
import { useTranslation } from "react-i18next";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import {
  useDivisionLabel,
  usePivotCoordinates,
} from "@/ui/feature/division/pivot.ts";
import { FADE } from "@/ui/shared/component/motion/transition.ts";
import { HaloText } from "./halo.tsx";
import {
  alpha,
  color,
  SECONDARY,
  textStyle,
  weight,
} from "@/ui/shared/component/theme.ts";

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
