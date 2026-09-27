// @pxl/web/feature/map/ui/control/scale-bar.tsx

import { StyleSheet } from "react-native";
import { G, Path, Svg } from "react-native-svg";

import { useKernel } from "@/kernel/shell.ts";

import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE, SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { Motion } from "@platform/motion.tsx";
import { HaloText } from "@/shared/ui/halo.tsx";
import { color, halo, haloRings, textStyle, weight } from "@/shared/ui/theme.ts";

/** How far the end ticks drop from the line. */
const TICK_PX = 5;
const LINE_PX = 1;
/** Room around the strokes for their halo. */
const PAD_PX = LINE_PX + halo.reach + 1;
/** From the bottom of the label's line box down to the line. */
const LABEL_GAP_PX = 6;

/** How far a distance on the map spans on screen, at the camera's zoom.
 *
 * Drawn as a drawing's dimension is: the label over a line with a tick
 * dropping at each end. A scale is read by its ends, and the halo softens
 * where a bare line stops.
 * The line and ticks are one path, stroked as the halo's rings, so the halo
 * runs unbroken where they meet. */
export function MapScaleBar({ show }: { show: boolean }) {
  const { total_meters, bar_width_px } = useKernel((s) => s.scale_bar);
  // Dimmed with the chrome around it, as a surface of its own.
  const veil = useVeil(1, SNAP);

  // SI units are case-sensitive: m (meter) != M (mega), km != KM — the
  // label must not be forced to upper case.
  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;

  const width = bar_width_px + PAD_PX * 2;
  // A one-pixel line centered on a pixel, spanning the bar's pixels inclusive.
  const left = PAD_PX + 0.5;
  const right = left + bar_width_px - 1;
  const d = `M${left} ${PAD_PX + TICK_PX}V${PAD_PX}H${right}V${PAD_PX + TICK_PX}`;
  const svgHeight = PAD_PX + TICK_PX + PAD_PX;
  // Centered in the title's slot, the line stands on the round controls'
  // centerline either side, the label above it and the ticks below: room
  // below balances the box about the line.
  const line = styles.label.lineHeight + LABEL_GAP_PX;
  const height = line + TICK_PX + PAD_PX;

  return (
    <Presence
      show={show}
      from={{ opacity: 0 }}
      exit={{ opacity: 0 }}
      transition={SETTLE}
      exitTransition={LEAVE}
    >
      <Motion to={{ opacity: veil.level }} transition={veil.transition} style={[styles.root, { paddingBottom: 2 * line - height }]}>
        <HaloText style={styles.label}>{label}</HaloText>
        <Svg width={width} height={svgHeight} viewBox={`0 0 ${width} ${svgHeight}`} style={styles.bar}>
          {haloRings.map((px) => (
            <G key={px} opacity={halo.ring}>
              <Path d={d} stroke={color.black} strokeWidth={LINE_PX + 2 * px} fill="none" strokeLinecap="square" />
            </G>
          ))}
          <Path d={d} stroke={color.white} strokeWidth={LINE_PX} fill="none" strokeLinecap="square" />
        </Svg>
      </Motion>
    </Presence>
  );
}

const styles = StyleSheet.create({
  root: { alignItems: "center" },
  label: { ...textStyle.caption2, fontWeight: weight.semibold, letterSpacing: 0.8, fontVariant: ["tabular-nums"], color: color.white },
  bar: { marginTop: LABEL_GAP_PX - PAD_PX },
});
