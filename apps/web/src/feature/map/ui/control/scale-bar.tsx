// @pxl/web/feature/map/ui/control/scale-bar.tsx

import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

import { Presence } from "@/shared/ui/motion/presence.tsx";
import { after, ARRIVE, SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { Motion } from "@platform/motion.tsx";
import { color, font, textEdge } from "@/shared/ui/theme.ts";

const TICK_PX = 6;
const LINE_PX = 1.5;
/** The dark edge each stroke is drawn on, so the bar reads over any ground. */
const EDGE_PX = 1;
const EDGE = "rgba(0, 0, 0, 0.9)";

/** How far a distance on the map spans on screen, at the camera's zoom; a
 * tap puts the zoom back. Hidden at the default zoom. */
export function MapScaleBar() {
  const { t } = useTranslation();
  const { total_meters, bar_width_px, is_default_zoom } = useKernel((s) => s.scale_bar);
  // Dimmed with the chrome around it, as a surface of its own.
  const veil = useVeil(1, SNAP);

  // SI units are case-sensitive: m (meter) != M (mega), km != KM — the
  // label must not be forced to upper case.
  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;

  return (
    <Presence
      show={!is_default_zoom}
      from={{ opacity: 0, translateY: 15 }}
      exit={{ opacity: 0, translateY: 10 }}
      transition={after(ARRIVE, 200)}
      exitTransition={ARRIVE}
    >
      <Pressable
        onPress={() => kernel_command(Command.INPUT_RESET_DIST)}
        accessibilityRole="button"
        accessibilityLabel={t("map.scale_bar.reset_zoom")}
      >
        {({ pressed }) => (
          <Motion to={{ scale: pressed ? 0.95 : 1, opacity: veil.level }} transition={veil.transition} style={styles.root}>
            <View style={[styles.bar, { width: bar_width_px }]}>
              <View style={[styles.stroke, styles.tick, { left: 0 }]} />
              <View style={[styles.stroke, styles.line]} />
              <View style={[styles.stroke, styles.tick, { right: 0 }]} />
            </View>
            <Text style={styles.label}>{label}</Text>
          </Motion>
        )}
      </Pressable>
    </Presence>
  );
}

const styles = StyleSheet.create({
  root: { alignItems: "center", paddingTop: 4, paddingBottom: 8, minWidth: 44 },
  bar: { height: TICK_PX + 2 * EDGE_PX },
  stroke: { position: "absolute", backgroundColor: color.white, borderWidth: EDGE_PX, borderColor: EDGE, boxSizing: "content-box" },
  tick: { top: 0, width: LINE_PX, height: TICK_PX },
  line: { left: 0, right: 0, top: TICK_PX / 2 - LINE_PX / 2, height: LINE_PX },
  label: {
    marginTop: 1,
    fontFamily: font.family,
    fontSize: 8,
    fontWeight: "700",
    letterSpacing: 0.8,
    color: color.white,
    ...textEdge(EDGE),
  },
});
