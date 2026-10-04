// @pxl/web/feature/map/ui/control/compass.tsx

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";
import { Path, Svg } from "react-native-svg";

import { Command, CompassState } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

import { LABEL, NORTH } from "@platform/palette.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { chrome } from "@/shared/ui/theme.ts";
import type { f64 } from "@xpute/core/abi/word.ts";

const DIAL = chrome.control;
const NEEDLE_HALF = 20;
const NEEDLE_WIDTH = 10;
const LIT = 0.75;
const NEEDLE_ROUND = 2;
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
function face(end: f64, side: f64): string {
  const ridge = end - Math.sign(end) * NEEDLE_ROUND;
  const turn = (side * NEEDLE_ROUND) / Math.abs(end);
  const edge = Math.hypot(side, end);
  const [x, y] = [turn + (side / edge) * Math.abs(turn), ridge - (end / edge) * Math.abs(turn)];
  return `M0 0L${side} 0L${x} ${y}Q${turn} ${ridge} 0 ${ridge}Z`;
}

/** So a momentary turn past the threshold does not flash the button. */
const HOLD_MS = SETTLE.durationMs;

function useHeld(value: boolean, ms: f64): boolean {
  const [held, setHeld] = useState(value);
  useEffect(() => {
    if (value === held) return;
    const timer = setTimeout(() => setHeld(value), ms);
    return () => clearTimeout(timer);
  }, [value, held, ms]);
  return held;
}

export function MapCompass() {
  const { t } = useTranslation();
  const azimuth = useKernel((s) => s.azimuth);
  const compass = useKernel((s) => s.compass);
  const shown = useHeld(compass !== CompassState.Hidden, HOLD_MS);
  const turn = -azimuth;
  const w = NEEDLE_WIDTH / 2;

  return (
    <Presence
      show={shown}
      // Never faded: the glass's style steps read as darkening. Not scale 0,
      // which UIKit cannot invert to hit-test.
      from={{ scale: 0.01, rotate: -45, translateY: 10 }}
      exit={{ scale: 0.01, rotate: 45 }}
      transition={SETTLE}
      exitTransition={LEAVE}
    >
      <CircularButton
        onPress={() => kernel_command(Command.INPUT_COMPASS_RESET)}
        label={t("map.controls.reset_orientation")}
        disabled={compass === CompassState.Following}
      >
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
              <Path d={face(-NEEDLE_HALF, -w)} fill={NORTH} fillOpacity={LIT} />
              <Path d={face(-NEEDLE_HALF, w)} fill={NORTH} />
              <Path d={face(NEEDLE_HALF, -w)} fill={LABEL.primary} fillOpacity={LIT} />
              <Path d={face(NEEDLE_HALF, w)} fill={LABEL.primary} />
            </Svg>
          </View>
        </View>
      </CircularButton>
    </Presence>
  );
}
