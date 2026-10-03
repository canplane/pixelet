// @pxl/web/feature/map/ui/control/scale-bar.tsx

import { StyleSheet } from "react-native";
import { G, Path, Svg } from "react-native-svg";

import type { f64 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { useKernel } from "@/kernel/shell.ts";

import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE, SNAP } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { Motion } from "@platform/motion.tsx";
import { HaloText } from "@/shared/ui/halo.tsx";
import { color, halo, haloRings, textStyle, weight } from "@/shared/ui/theme.ts";

const TICK_PX = 5;
const QUARTER_TICK_PX = 3;
const LINE_PX = 1;
const PAD_PX = LINE_PX + halo.reach + 1;
const LABEL_GAP_PX = 8;
const BAR_MAX_PX = 80;

/** Nearest 1, 2 or 5 × 10ⁿ. */
function niceLength(m: f64): f64 {
  if (!(m > 0)) return 0;
  const base = 10 ** Math.floor(Math.log10(m));
  const f = m / base;
  return (f < 1.5 ? 1 : f < 3.5 ? 2 : f < 7.5 ? 5 : 10) * base;
}

/** Line and ticks are one path, stroked as the halo's rings, so the halo is unbroken where they meet. */
export function MapScaleBar({ show }: { show: boolean }) {
  const mPerPx = useKernel((s) => s.m_per_px);
  const totalMeters = niceLength(BAR_MAX_PX * mPerPx);
  const barWidthPx = mPerPx > 0 ? totalMeters / mPerPx : 0;
  const veil = useVeil(1, SNAP);

  // SI units are case-sensitive: never upper-case the label.
  const label: string = totalMeters >= 1000 ? `${totalMeters / 1000}km` : `${totalMeters}m`;

  const width = barWidthPx + PAD_PX * 2;
  const left = PAD_PX + 0.5;
  const right = left + barWidthPx - 1;
  const at = (f: f64) => round(left + (right - left) * f - 0.5) + 0.5;
  const tick = (f: f64, px: f64) => `M${at(f)} ${PAD_PX}V${PAD_PX + px}`;
  const d = `M${left} ${PAD_PX + TICK_PX}V${PAD_PX}H${right}V${PAD_PX + TICK_PX}` + tick(0.5, TICK_PX) + tick(0.25, QUARTER_TICK_PX) + tick(0.75, QUARTER_TICK_PX);
  const svgHeight = PAD_PX + TICK_PX + PAD_PX;
  const line = halo.reach + styles.label.lineHeight + LABEL_GAP_PX;
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
  bar: { marginTop: LABEL_GAP_PX - halo.reach - PAD_PX },
});
