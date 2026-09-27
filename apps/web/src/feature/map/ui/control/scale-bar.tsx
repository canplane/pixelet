// @pxl/web/feature/map/ui/control/scale-bar.tsx

import { StyleSheet } from "react-native";
import { Path, Svg, Text as SvgText } from "react-native-svg";

import { useKernel } from "@/kernel/shell.ts";

import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE, SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { Motion } from "@platform/motion.tsx";
import { color, edge, font } from "@/shared/ui/theme.ts";

const TICK_PX = 8;
/** Room around the strokes for their edge. */
const PAD_PX = 3;
const LABEL_PX = 11;
/** Where the label's baseline stands below the bar's box: tucked up under
 * it, as a label sits under a map's scale. */
const LABEL_BASELINE_PX = 7;
/** The narrowest the bar is, so a short one still has room for its label. */
const MIN_WIDTH_PX = 44;

/** How far a distance on the map spans on screen, at the camera's zoom.
 *
 * Drawn as one path stroked twice — the edge wide and dark, the line over it
 * — so the edge runs unbroken round the ticks and the line where they meet,
 * and the label as text stroked the same way: a crisp outline on every
 * platform, where a device's text takes one soft shadow. */
export function MapScaleBar({ show }: { show: boolean }) {
  const { total_meters, bar_width_px } = useKernel((s) => s.scale_bar);
  // Dimmed with the chrome around it, as a surface of its own.
  const veil = useVeil(1, SNAP);

  // SI units are case-sensitive: m (meter) != M (mega), km != KM — the
  // label must not be forced to upper case.
  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;

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
      show={show}
      from={{ opacity: 0 }}
      exit={{ opacity: 0 }}
      transition={SETTLE}
      exitTransition={LEAVE}
    >
      <Motion to={{ opacity: veil.level }} transition={veil.transition} style={styles.root}>
        <Svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
          <Path d={d} stroke={edge} strokeWidth={3.5} fill="none" strokeLinecap="square" />
          <Path d={d} stroke={color.white} strokeWidth={1.5} fill="none" strokeLinecap="square" />
          <SvgText {...text} stroke={edge} strokeWidth={2} strokeLinejoin="round" fill={edge}>{label}</SvgText>
          <SvgText {...text} fill={color.white}>{label}</SvgText>
        </Svg>
      </Motion>
    </Presence>
  );
}

const styles = StyleSheet.create({
  root: { alignItems: "center", paddingTop: 4, paddingBottom: 8 },
});
