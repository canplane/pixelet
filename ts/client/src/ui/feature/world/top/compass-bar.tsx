// @pxl/client/ui/feature/world/top/compass-bar.tsx

import { useState } from "react";
import { StyleSheet, View } from "react-native";
import { G } from "react-native-svg";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round, wrap } from "@xpute/kit/math/scalar.ts";

import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/ui/shared/component/motion/transition.ts";
import {
  COLOR,
  GAUGE_FACE,
  GAUGE_TRACKING_EM,
  TEXT_STYLE,
} from "@/ui/shared/component/theme.ts";
import { useView } from "@/ui/shared/watch.ts";
import { HaloPath, HaloSvg, HaloText } from "./halo.tsx";

/** As wide as the strip, up to the scrim's flat width. */
const COMPASS_MAX_PX = 560;
const PX_PER_DEG = 2.6;
const STEP_DEG = 15;
const TICK_PX = 4;
const MAJOR_TICK_PX = 6;
const LABEL_WIDTH_PX = 32;
/** The labels' line. */
const LINE_HEIGHT_PX = 20;
/** Between the labels' line and the ticks, so that a direction stands 6
 * pixels above its tick, as the scale's label stands below its own. */
const LABEL_GAP_PX = 1;
const DIR_FONT_PX = 14;
const DEGREE_FONT_PX = 12;
/** The gauges' face's ascent less its descent, as a share of its size. */
const ASCENT_LESS_DESCENT = 0.708;
const DIRECTIONS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

/** A mark every `STEP_DEG`, where `azimuthDeg` lies at the middle of a
 * bar `width` wide: its bearing, where it stands and how present it is, gone at
 * the ends. */
function marks(
  azimuthDeg: f64,
  width: f64,
): { deg: f64; x: f64; opacity: f64 }[] {
  const half = width / 2;
  const out = [];
  for (let deg = 0; deg < 360; deg += STEP_DEG) {
    const off = wrap(deg - azimuthDeg, -180, 180) * PX_PER_DEG;
    if (Math.abs(off) < half) {
      out.push({ deg, x: half + off, opacity: 1 - (off / half) ** 2 });
    }
  }
  return out;
}

/** The tick of the bearing `deg` at `x`. Ticks stand on one line, and a
 * square cap ends each half a pixel past its end, so that a direction's fills
 * `MAJOR_TICK_PX`. */
function tick(deg: f64, x: f64): string {
  const px = deg % 45 === 0 ? MAJOR_TICK_PX : TICK_PX;
  const at = round(x - 0.5) + 0.5;
  return `M${at} ${MAJOR_TICK_PX - px + 0.5}V${MAJOR_TICK_PX - 0.5}`;
}

/** The view's azimuth as a compass bar: the direction the view faces lies
 * at its middle. */
export function CompassBar({ show }: { show: boolean }) {
  const azimuthDeg = (useView((v) => v.azimuth) * 180) / Math.PI;
  const [width, setWidth] = useState(0);
  const shown = marks(azimuthDeg, width);

  return (
    <Presence
      show={show}
      from={{ opacity: 0 }}
      exit={{ opacity: 0 }}
      transition={SETTLE}
      exitTransition={LEAVE}
      style={STYLE.root}
    >
      <View
        style={STYLE.labels}
        onLayout={(e) => setWidth(e.nativeEvent.layout.width)}
      >
        {shown.map(({ deg, x, opacity }) => (
          <View
            key={deg}
            style={[STYLE.label, { left: x - LABEL_WIDTH_PX / 2, opacity }]}
          >
            <HaloText style={deg % 45 === 0 ? BAR_LABEL.dir : BAR_LABEL.degree}>
              {deg % 45 === 0 ? DIRECTIONS[deg / 45] : `${deg}`}
            </HaloText>
          </View>
        ))}
      </View>
      <HaloSvg width={width} height={MAJOR_TICK_PX}>
        {shown.map(({ deg, x, opacity }) => (
          <G key={deg} opacity={opacity}>
            <HaloPath d={tick(deg, x)} width={1} />
          </G>
        ))}
      </HaloSvg>
    </Presence>
  );
}

const STYLE = StyleSheet.create({
  root: { width: "100%", maxWidth: COMPASS_MAX_PX, alignItems: "center" },
  labels: {
    alignSelf: "stretch",
    height: LINE_HEIGHT_PX,
    marginBottom: LABEL_GAP_PX,
  },
  label: {
    position: "absolute",
    top: 0,
    bottom: 0,
    width: LABEL_WIDTH_PX,
    alignItems: "center",
    justifyContent: "flex-end",
  },
});

const BAR_LABEL = StyleSheet.create({
  dir: {
    ...TEXT_STYLE.title,
    fontSize: DIR_FONT_PX,
    lineHeight: LINE_HEIGHT_PX,
    ...GAUGE_FACE,
    letterSpacing: DIR_FONT_PX * GAUGE_TRACKING_EM,
    color: COLOR.white,
  },
  degree: {
    ...TEXT_STYLE.caption2,
    // Both stand on the bottom of the line, and a smaller text's baseline
    // lies higher in a line as high; a line this much lower brings it down
    // to the directions'.
    fontSize: DEGREE_FONT_PX,
    lineHeight:
      LINE_HEIGHT_PX - (DIR_FONT_PX - DEGREE_FONT_PX) * ASCENT_LESS_DESCENT,
    ...GAUGE_FACE,
    letterSpacing: DEGREE_FONT_PX * GAUGE_TRACKING_EM,
    fontVariant: ["tabular-nums"],
    color: COLOR.white,
  },
});
