// @pxl/client/ui/feature/world/top/scale-bar.tsx

import { StyleSheet, View } from "react-native";
import { Svg } from "react-native-svg";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { HaloPath, HaloText } from "./halo.tsx";
import {
  COLOR,
  HALO,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";
import { useView } from "@/ui/shared/watch.ts";

const TICK_PX = 5;
const QUARTER_TICK_PX = 3;
const LINE_PX = 1;
const PAD_PX = LINE_PX + HALO.width + 1;
const LABEL_GAP_PX = 8;
const BAR_MAX_PX = 80;

/** The length nearest to `m` of the form 1, 2 or 5 × 10ⁿ. */
function nice_length(m: f64): f64 {
  if (!(m > 0)) return 0;
  const base = 10 ** Math.floor(Math.log10(m));
  const f = m / base;
  return (f < 1.5 ? 1 : f < 3.5 ? 2 : f < 7.5 ? 5 : 10) * base;
}

/** The line and its ticks are one path under one halo, so that the halo is no
 * darker where they meet. */
export function MapScaleBar({ show }: { show: boolean }) {
  const m_per_px = useView((v) => v.m_per_px);
  const total_m = nice_length(BAR_MAX_PX * m_per_px);
  const bar_width_px = m_per_px > 0 ? total_m / m_per_px : 0;

  // SI units are case-sensitive: never upper-case the label.
  const label: string = total_m >= 1000 ? `${total_m / 1000}km` : `${total_m}m`;

  const width = bar_width_px + PAD_PX * 2;
  const left = PAD_PX + 0.5;
  const right = left + bar_width_px - 1;
  const at = (f: f64) => round(left + (right - left) * f - 0.5) + 0.5;
  const tick = (f: f64, px: f64) => `M${at(f)} ${PAD_PX}V${PAD_PX + px}`;
  const d =
    `M${left} ${PAD_PX + TICK_PX}V${PAD_PX}H${right}V${PAD_PX + TICK_PX}` +
    tick(0.5, TICK_PX) +
    tick(0.25, QUARTER_TICK_PX) +
    tick(0.75, QUARTER_TICK_PX);
  const svg_height_px = PAD_PX + TICK_PX + PAD_PX;

  return (
    <Presence
      show={show}
      from={{ opacity: 0 }}
      exit={{ opacity: 0 }}
      transition={SETTLE}
      exitTransition={LEAVE}
      style={styles.root}
    >
      <View style={styles.above}>
        <HaloText style={scale_label}>{label}</HaloText>
      </View>
      <Svg
        width={width}
        height={svg_height_px}
        viewBox={`0 0 ${width} ${svg_height_px}`}
      >
        <HaloPath d={d} width={LINE_PX} />
      </Svg>
    </Presence>
  );
}

const styles = StyleSheet.create({
  // The bar is as tall below its line as above it, so that the line lies at
  // the strip's middle.
  root: { alignItems: "center", paddingTop: TICK_PX },
  above: {
    position: "absolute",
    left: 0,
    right: 0,
    bottom: PAD_PX + TICK_PX + LABEL_GAP_PX - HALO.width,
    alignItems: "center",
  },
});

const scale_label = StyleSheet.create({
  label: {
    ...TEXT_STYLE.caption2,
    fontWeight: WEIGHT.semibold,
    letterSpacing: 0.8,
    fontVariant: ["tabular-nums"],
    color: COLOR.white,
  },
}).label;
