// @pxl/client/shell/world/top/current-address.tsx

import { StyleSheet } from "react-native";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import { useCenterCoordinates, useDivisionLabel } from "@/shell/division/focus.ts";
import { FADE } from "@/shell/ui/motion/transition.ts";
import { HaloText } from "./halo.tsx";
import { color, textStyle, weight } from "@/shell/ui/theme.ts";

/** Coordinates stand in where the center is in no place (sea, or not yet arrived). */
export function CurrentAddress() {
  const label = useDivisionLabel();
  const position = useCenterCoordinates();

  const text = label ?? position;
  if (!text) return null;

  return (
    <Motion key={label ?? ""} from={{ opacity: 0 }} to={{}} transition={FADE} style={styles.shrink}>
      <HaloText style={[styles.label, !label && styles.numbers]}>{text}</HaloText>
    </Motion>
  );
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.body, fontWeight: weight.semibold, color: color.white },
  numbers: { fontVariant: ["tabular-nums"] },
});
