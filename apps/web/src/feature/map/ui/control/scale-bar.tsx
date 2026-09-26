// @pxl/web/feature/map/ui/control/scale-bar.tsx

import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet } from "react-native";
import { Path, Svg, Text as SvgText } from "react-native-svg";

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

import { Presence } from "@/shared/ui/motion/presence.tsx";
import { after, ARRIVE, SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { Motion } from "@platform/motion.tsx";
import { color, font } from "@/shared/ui/theme.ts";

const TICK_PX = 6;
/** Room around the strokes for their edge. */
const PAD_PX = 3;
const LABEL_PX = 8;
/** Where the label's baseline stands below the bar's box: tucked up under
 * it, as a label sits under a map's scale. */
const LABEL_BASELINE_PX = 5;
/** The narrowest the control is, so a short bar still has a label and a tap
 * target under it. */
const MIN_WIDTH_PX = 44;
/** The dark edge every stroke and the label are drawn on, so the bar reads
 * over any ground. */
const EDGE = "rgba(0, 0, 0, 0.9)";
/** The bar as it leaves, the zoom back at its default: the one moment it is
 * shown in a color, which says what the tap did. */
const RESET = "#ffb900";

/** How far a distance on the map spans on screen, at the camera's zoom; a
 * tap puts the zoom back. Hidden at the default zoom.
 *
 * Drawn as one path stroked twice — the edge wide and dark, the line over it
 * — so the edge runs unbroken round the ticks and the line where they meet,
 * and the label as text stroked the same way: a crisp outline on every
 * platform, where a device's text takes one soft shadow. */
export function MapScaleBar() {
  const { t } = useTranslation();
  const { total_meters, bar_width_px, is_default_zoom } = useKernel((s) => s.scale_bar);
  // Dimmed with the chrome around it, as a surface of its own.
  const veil = useVeil(1, SNAP);

  // SI units are case-sensitive: m (meter) != M (mega), km != KM — the
  // label must not be forced to upper case.
  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;
  const ink = is_default_zoom ? RESET : color.white;

  const width = Math.max(bar_width_px + PAD_PX * 2, MIN_WIDTH_PX);
  const barHeight = TICK_PX + PAD_PX * 2;
  const height = barHeight + LABEL_BASELINE_PX + LABEL_PX / 2;
  const left = (width - bar_width_px) / 2 + 0.5;
  const right = left + bar_width_px - 1;
  const top = PAD_PX;
  const bottom = PAD_PX + TICK_PX;
  const mid = top + TICK_PX / 2;
  const d = `M${left} ${top}V${bottom} M${left} ${mid}H${right} M${right} ${top}V${bottom}`;
  const text = { x: width / 2, y: barHeight + LABEL_BASELINE_PX, fontSize: LABEL_PX, fontWeight: "700", fontFamily: font.family, letterSpacing: 0.8, textAnchor: "middle" } as const;

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
            <Svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
              <Path d={d} stroke={EDGE} strokeWidth={3.5} fill="none" strokeLinecap="square" />
              <Path d={d} stroke={ink} strokeWidth={1.5} fill="none" strokeLinecap="square" />
              <SvgText {...text} stroke={EDGE} strokeWidth={2} strokeLinejoin="round" fill={EDGE}>{label}</SvgText>
              <SvgText {...text} fill={ink}>{label}</SvgText>
            </Svg>
          </Motion>
        )}
      </Pressable>
    </Presence>
  );
}

const styles = StyleSheet.create({
  root: { alignItems: "center", paddingTop: 4, paddingBottom: 8 },
});
