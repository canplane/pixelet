// @pxl/web/feature/map/ui/control/compass.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { alpha, color, font } from "@/shared/ui/theme.ts";

export function MapCompass() {
  const { t } = useTranslation();
  const azimuth = useKernel((s) => s.azimuth);
  const showCompass = useKernel((s) => s.show_compass);

  if (!showCompass) return null;

  return (
    <CircularButton onPress={() => kernel_command(Command.INPUT_COMPASS_RESET)} label={t("map.controls.reset_orientation")}>
      <Text style={styles.north}>N</Text>
      <View style={[styles.needle, { transform: [{ rotate: `${(-azimuth * 180) / Math.PI}deg` }] }]}>
        <View style={styles.tip} />
        <View style={styles.tail} />
      </View>
    </CircularButton>
  );
}

const styles = StyleSheet.create({
  north: {
    position: "absolute",
    top: 2,
    fontFamily: font.family,
    fontSize: 10,
    fontWeight: "500",
    letterSpacing: -0.5,
    color: alpha(color.ink, 0.5),
  },
  needle: { width: 2, height: 24 },
  tip: { flex: 1, backgroundColor: alpha(color.red500, 0.8), borderTopLeftRadius: 1, borderTopRightRadius: 1, boxShadow: `0 0 12px ${alpha(color.red500, 0.4)}` },
  tail: { flex: 1, backgroundColor: alpha(color.ink, 0.3), borderBottomLeftRadius: 1, borderBottomRightRadius: 1 },
});
