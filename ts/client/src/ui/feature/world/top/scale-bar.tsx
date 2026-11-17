// @pxl/client/ui/feature/world/top/scale-bar.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { HaloPath, HaloSvg, HaloText } from "./halo.tsx";
import {
  COLOR,
  GAUGE_FACE,
  GAUGE_TRACKING_EM,
  TEXT_STYLE,
} from "@/ui/shared/component/theme.ts";
import { useView } from "@/ui/shared/watch.ts";

const TICK_PX = 5;
const QUARTER_TICK_PX = 3;
const LINE_PX = 1;
/** Between the ticks and the label's line, so that the label's digits stand 6
 * pixels below the ticks, as a compass direction stands above its own. */
const LABEL_GAP_PX = 3.5;
const BAR_MAX_PX = 80;
const LABEL_FONT_PX = 11;

/** The length nearest to `m` of the form 1, 2 or 5 × 10ⁿ. */
function niceLength(m: f64): f64 {
  if (!(m > 0)) return 0;
  const base = 10 ** Math.floor(Math.log10(m));
  const f = m / base;
  return (f < 1.5 ? 1 : f < 3.5 ? 2 : f < 7.5 ? 5 : 10) * base;
}

/** What the bar draws where a logical pixel spans `mPerPx` meters: its
 * width, 0 where there is no scale, and the length it stands for. */
function scaleOf(mPerPx: f64): { width: f64; label: string } {
  const totalM = niceLength(BAR_MAX_PX * mPerPx);
  // SI units are case-sensitive: never upper-case the label.
  return {
    width: mPerPx > 0 ? round(totalM / mPerPx) : 0,
    label: totalM >= 1000 ? `${totalM / 1000}km` : `${totalM}m`,
  };
}

/** The line and its ticks hang above its length. They are one path under
 * one halo, so that the halo is no darker where they meet. It shows only
 * where there is a scale. */
export function MapScaleBar({ show }: { show: boolean }) {
  const { width, label } = scaleOf(useView((v) => v.mPerPx));

  // The line lies along the first row of pixels, and the ticks hang from it.
  // A square cap ends each stroke half a pixel past its end, so that the
  // drawing fills `width` by `TICK_PX`.
  const [left, right] = [0.5, width - 0.5];
  const at = (f: f64) => round(left + (right - left) * f - 0.5) + 0.5;
  const tick = (f: f64, px: f64) => `M${at(f)} 0.5V${px - 0.5}`;
  const d =
    `M${left} ${TICK_PX - 0.5}V0.5H${right}V${TICK_PX - 0.5}` +
    tick(0.5, TICK_PX) +
    tick(0.25, QUARTER_TICK_PX) +
    tick(0.75, QUARTER_TICK_PX);

  return (
    <Presence
      show={show && width > 0}
      from={{ opacity: 0 }}
      exit={{ opacity: 0 }}
      transition={SETTLE}
      exitTransition={LEAVE}
      style={STYLE.root}
    >
      <HaloSvg width={width} height={TICK_PX}>
        <HaloPath d={d} width={LINE_PX} />
      </HaloSvg>
      <View style={STYLE.below}>
        <HaloText style={SCALE_LABEL}>{label}</HaloText>
      </View>
    </Presence>
  );
}

const STYLE = StyleSheet.create({
  root: { alignItems: "center" },
  below: { marginTop: LABEL_GAP_PX },
});

const SCALE_LABEL = StyleSheet.create({
  label: {
    ...TEXT_STYLE.caption2,
    fontSize: LABEL_FONT_PX,
    lineHeight: 13,
    ...GAUGE_FACE,
    letterSpacing: LABEL_FONT_PX * GAUGE_TRACKING_EM,
    fontVariant: ["tabular-nums"],
    color: COLOR.white,
  },
}).label;
