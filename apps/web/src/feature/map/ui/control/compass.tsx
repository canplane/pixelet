// @pxl/web/feature/map/ui/control/compass.tsx

import { StyleSheet, View } from "react-native";
import { Path, Svg } from "react-native-svg";

import { useKernel } from "@/kernel/shell.ts";

import { LABEL, NORTH } from "@platform/palette.ts";
import { chrome } from "@/shared/ui/theme.ts";
import type { f64 } from "@xpute/core/abi/word.ts";

const DIAL = chrome.control;
const NEEDLE_HALF = 20;
const NEEDLE_WIDTH = 10;
const LIT = 0.75;
const NEEDLE_ROUND = 2;
/** How far toward south the halves' seam bends at the middle, so the two
 * colors meet in a chevron, as a physical needle's bevel does. */
const SEAM_BEND = 3;
const TICK_TO = DIAL / 2 - 3;
const TICK_FROM = TICK_TO - 2.5;
const QUARTER_STROKE = 1.5;
const EIGHTH_STROKE = 0.75;
const [QUARTERS, EIGHTHS] = [0, 1].map((odd) =>
  Array.from({ length: 4 }, (_, i) => {
    const a = ((2 * i + odd) * Math.PI) / 4;
    const [x, y] = [Math.sin(a), -Math.cos(a)];
    return `M${TICK_FROM * x} ${TICK_FROM * y}L${TICK_TO * x} ${TICK_TO * y}`;
  }).join("")
);
const BOX = `${-DIAL / 2} ${-DIAL / 2} ${DIAL} ${DIAL}`;

/** One face of the needle; its mirror meets it with the same tangent, making one rounded end. */
function face(end: f64, side: f64, seam: f64): string {
  const ridge = end - Math.sign(end) * NEEDLE_ROUND;
  const turn = (side * NEEDLE_ROUND) / Math.abs(end);
  const edge = Math.hypot(side, end);
  const [x, y] = [turn + (side / edge) * Math.abs(turn), ridge - (end / edge) * Math.abs(turn)];
  return `M0 ${seam}L${side} 0L${x} ${y}Q${turn} ${ridge} 0 ${ridge}Z`;
}

/** The dial and a needle pointing north, turned against the map. */
export function CompassFace() {
  const turn = -useKernel((s) => s.azimuth);
  const w = NEEDLE_WIDTH / 2;
  return (
    <View style={{ width: DIAL, height: DIAL }}>
      <Svg width={DIAL} height={DIAL} viewBox={BOX} style={StyleSheet.absoluteFill}>
        <Path d={QUARTERS} stroke={LABEL.secondary} strokeWidth={QUARTER_STROKE} strokeLinecap="round" />
        <Path d={EIGHTHS} stroke={LABEL.tertiary} strokeWidth={EIGHTH_STROKE} strokeLinecap="round" />
      </Svg>
      {
        // RN's transform: a device's SVG refuses a CSS string.
      }
      <View style={[StyleSheet.absoluteFill, { transform: [{ rotate: `${turn}rad` }] }]}>
        <Svg width={DIAL} height={DIAL} viewBox={BOX}>
          <Path d={face(-NEEDLE_HALF, -w, SEAM_BEND)} fill={NORTH} fillOpacity={LIT} />
          <Path d={face(-NEEDLE_HALF, w, SEAM_BEND)} fill={NORTH} />
          <Path d={face(NEEDLE_HALF, -w, SEAM_BEND)} fill={LABEL.primary} fillOpacity={LIT} />
          <Path d={face(NEEDLE_HALF, w, SEAM_BEND)} fill={LABEL.primary} />
        </Svg>
      </View>
    </View>
  );
}
