// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { StyleSheet, Text, useWindowDimensions } from "react-native";

import { alpha, color, font } from "@/shared/ui/theme.ts";

interface CurrentAddressProps {
  label: string | null;
}

/** The place under the map's center. */
export function CurrentAddress(props: CurrentAddressProps) {
  const { label } = props;
  const { width } = useWindowDimensions();

  if (!label) return null;

  return (
    <Text numberOfLines={1} style={[styles.label, { maxWidth: width * 0.4 }]}>
      {label}
    </Text>
  );
}

const styles = StyleSheet.create({
  label: { fontFamily: font.family, fontSize: 12, lineHeight: 16, fontWeight: "500", color: alpha(color.ink, 0.8), flexShrink: 1 },
});
