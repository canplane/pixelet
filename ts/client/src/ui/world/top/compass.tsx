// @pxl/client/ui/world/top/compass.tsx

import { StyleSheet, View } from "react-native";
import { Path, Svg } from "react-native-svg";

import { LABEL, NORTH } from "@/ui/component/palette.ts";
import { chrome, SECONDARY } from "@/ui/component/theme.ts";
import type { f64 } from "@xpute/kit/abi/word.ts";
import { useView } from "@/ui/watch.ts";

const NEEDLE_HALF = 16;
const NEEDLE_WIDTH = 9;
const NEEDLE_ROUND = 2;
/** How far toward south the seam between the needle's halves bends at the
 * middle, so that the two colors meet in a chevron, as on a physical needle's
 * bevel. */
const SEAM_BEND = 2;
const TICK_TO = chrome.control / 2 - 3;
const TICK_FROM = TICK_TO - 2.5;
const QUARTER_STROKE = 1.5;
const EIGHTH_STROKE = 0.75;
const [QUARTERS, EIGHTHS] = [0, 1].map((odd) =>
  Array.from({ length: 4 }, (_, i) => {
    const a = ((2 * i + odd) * Math.PI) / 4;
    const [x, y] = [Math.sin(a), -Math.cos(a)];
    return `M${TICK_FROM * x} ${TICK_FROM * y}L${TICK_TO * x} ${TICK_TO * y}`;
  }).join(""),
);
const BOX = `${-chrome.control / 2} ${
  -chrome.control / 2
} ${chrome.control} ${chrome.control}`;

/** One face of the needle. Its mirror image meets it with the same tangent, so
 * the two make one rounded end. */
function face(end: f64, side: f64, seam: f64): string {
  const ridge = end - Math.sign(end) * NEEDLE_ROUND;
  const turn = (side * NEEDLE_ROUND) / Math.abs(end);
  const edge = Math.hypot(side, end);
  const [x, y] = [
    turn + (side / edge) * Math.abs(turn),
    ridge - (end / edge) * Math.abs(turn),
  ];
  return `M0 ${seam}L${side} 0L${x} ${y}Q${turn} ${ridge} 0 ${ridge}Z`;
}

/** The dial and a needle that points north. The needle turns against the
 * view's azimuth. */
export function CompassFace() {
  const turn = -useView((v) => v.azimuth);
  const w = NEEDLE_WIDTH / 2;
  return (
    <View style={{ width: chrome.control, height: chrome.control }}>
      <Svg
        width={chrome.control}
        height={chrome.control}
        viewBox={BOX}
        style={StyleSheet.absoluteFill}
      >
        <Path
          d={QUARTERS}
          stroke={LABEL.secondary}
          strokeWidth={QUARTER_STROKE}
          strokeLinecap="round"
        />
        <Path
          d={EIGHTHS}
          stroke={LABEL.tertiary}
          strokeWidth={EIGHTH_STROKE}
          strokeLinecap="round"
        />
      </Svg>
      {
        // The needle turns by React Native's transform on a view, because a
        // device's SVG refuses a CSS transform string.
      }
      <View
        style={[
          StyleSheet.absoluteFill,
          {
            transform: [{ rotate: `${turn}rad` }],
          },
        ]}
      >
        <Svg width={chrome.control} height={chrome.control} viewBox={BOX}>
          {
            // Each half has a shaded face. The south half shades from the
            // primary label color to the secondary, and the red north half
            // shades by the same step, the `SECONDARY` opacity.
          }
          <Path
            d={face(-NEEDLE_HALF, -w, SEAM_BEND)}
            fill={NORTH}
            fillOpacity={SECONDARY}
          />
          <Path d={face(-NEEDLE_HALF, w, SEAM_BEND)} fill={NORTH} />
          <Path d={face(NEEDLE_HALF, -w, SEAM_BEND)} fill={LABEL.secondary} />
          <Path d={face(NEEDLE_HALF, w, SEAM_BEND)} fill={LABEL.primary} />
        </Svg>
      </View>
    </View>
  );
}
