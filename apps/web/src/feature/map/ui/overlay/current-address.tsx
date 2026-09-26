// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { StyleSheet, Text, useWindowDimensions } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { LABEL } from "@platform/palette.ts";
import { LEAVE } from "@/shared/ui/motion/transition.ts";
import { textStyle, weight } from "@/shared/ui/theme.ts";

interface CurrentAddressProps {
  label: string | null;
}

/** The place under the map's center; a new place fades in over the pill's
 * room for it, which a changed width has already set. */
export function CurrentAddress(props: CurrentAddressProps) {
  const { label } = props;
  const { width } = useWindowDimensions();

  if (!label) return null;

  return (
    <Motion key={label} from={{ opacity: 0 }} to={{}} transition={{ ...LEAVE, durationMs: 200 }} style={styles.shrink}>
      <Text numberOfLines={1} style={[styles.label, { maxWidth: width * 0.4 }]}>
        {label}
      </Text>
    </Motion>
  );
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.caption, fontWeight: weight.medium, color: LABEL.primary, flexShrink: 1 },
});
