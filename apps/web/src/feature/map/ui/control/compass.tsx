// @pxl/web/feature/map/ui/control/compass.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

import { LABEL } from "@platform/palette.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { after, ARRIVE } from "@/shared/ui/motion/transition.ts";
import { alpha, color, textStyle } from "@/shared/ui/theme.ts";

export function MapCompass() {
  const { t } = useTranslation();
  const azimuth = useKernel((s) => s.azimuth);
  const showCompass = useKernel((s) => s.show_compass);

  return (
    <Presence
      show={showCompass}
      from={{ scale: 0.8, rotate: -45, translateY: 10 }}
      exit={{ scale: 0.5, rotate: 45 }}
      transition={after(ARRIVE, 400)}
      exitTransition={ARRIVE}
    >
      {
        // Faded by its glass, not by the presence around it
        // (shared/ui/motion/veil.ts).
      }
      <CircularButton
        onPress={() => kernel_command(Command.INPUT_COMPASS_RESET)}
        label={t("map.controls.reset_orientation")}
        fade={showCompass ? 1 : 0}
        fadeFrom={0}
        fadeTransition={showCompass ? after(ARRIVE, 400) : ARRIVE}
      >
        <Text style={styles.north}>N</Text>
        <View style={[styles.needle, { transform: [{ rotate: `${(-azimuth * 180) / Math.PI}deg` }] }]}>
          <View style={styles.tip} />
          <View style={styles.tail} />
        </View>
      </CircularButton>
    </Presence>
  );
}

const styles = StyleSheet.create({
  north: {
    position: "absolute",
    top: 2,
    ...textStyle.caption2,
    letterSpacing: -0.5,
    color: LABEL.secondary,
  },
  needle: { width: 2, height: 24 },
  tip: { flex: 1, backgroundColor: alpha(color.north, 0.8), borderTopLeftRadius: 1, borderTopRightRadius: 1, boxShadow: `0 0 12px ${alpha(color.north, 0.4)}` },
  tail: { flex: 1, backgroundColor: LABEL.secondary, borderBottomLeftRadius: 1, borderBottomRightRadius: 1 },
});
