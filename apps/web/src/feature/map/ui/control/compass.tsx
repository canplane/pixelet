// @pxl/web/feature/map/ui/control/compass.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";
import { Path, Svg } from "react-native-svg";

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";

import { LABEL, NORTH } from "@platform/palette.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { after, ARRIVE } from "@/shared/ui/motion/transition.ts";
import { chrome } from "@/shared/ui/theme.ts";
import type { f64 } from "@xpute/core/abi/word.ts";

/** The face the needle is drawn on: the button's. */
const DIAL = chrome.control;
/** A lozenge, widest at the pivot and pointed at both ends, each half cut
 * into a lit face and a shaded one along its ridge, as a machined needle
 * catches the light. Its colors are the system's, as a system compass's are:
 * north its red, south the glyph color every other round control draws in. */
const NEEDLE_HALF = 20;
const NEEDLE_WIDTH = 10;
/** The lit face of each half is its shaded face's color let through this
 * much: one step lighter, the same hue. */
const LIT = 0.75;
/** How far back along the ridge from each point the needle's end turns
 * round. */
const NEEDLE_ROUND = 2;
/** Printed on the face: a hairline at every eighth of the turn, the four
 * quarters heavier and a step stronger than the eighths between them, both
 * weaker than the needle they stand behind. Each runs out to 3 inside the
 * button's rim. */
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

/** One face of the needle: from the pivot out to its corner at `side`, along
 * the edge towards the point at `end`, then round on a curve that meets the
 * ridge square, and back down the ridge. Its mirror face meets it there with
 * the same tangent, so the two curves make one smooth rounded end. The curve
 * leaves the edge where it is as far from the corner of the turn as the ridge
 * is, so it bends evenly. */
function face(end: f64, side: f64): string {
  const ridge = end - Math.sign(end) * NEEDLE_ROUND;
  const turn = (side * NEEDLE_ROUND) / Math.abs(end);
  const edge = Math.hypot(side, end);
  const [x, y] = [turn + (side / edge) * Math.abs(turn), ridge - (end / edge) * Math.abs(turn)];
  return `M0 0L${side} 0L${x} ${y}Q${turn} ${ridge} 0 ${ridge}Z`;
}

export function MapCompass() {
  const { t } = useTranslation();
  const azimuth = useKernel((s) => s.azimuth);
  const showCompass = useKernel((s) => s.show_compass);
  // Clockwise on the screen, as the view turns.
  const turn = -azimuth;
  const w = NEEDLE_WIDTH / 2;

  return (
    <Presence
      show={showCompass}
      // It turns in and grows from next to nothing, and turns on and shrinks
      // away. Its glass is never faded: the glass steps through a darker style
      // on its way in and out, which read as the button darkening. Not from
      // zero, whose transform has no inverse for UIKit to hit-test and draw
      // an effect view through.
      from={{ scale: 0.01, rotate: -45, translateY: 10 }}
      exit={{ scale: 0.01, rotate: 45 }}
      transition={after(ARRIVE, 400)}
      exitTransition={ARRIVE}
    >
      <CircularButton onPress={() => kernel_command(Command.INPUT_COMPASS_RESET)} label={t("map.controls.reset_orientation")}>
        <View style={{ width: DIAL, height: DIAL }}>
          <Svg width={DIAL} height={DIAL} viewBox={BOX} style={StyleSheet.absoluteFill}>
            <Path d={QUARTERS} stroke={LABEL.secondary} strokeWidth={QUARTER_STROKE} strokeLinecap="round" />
            <Path d={EIGHTHS} stroke={LABEL.tertiary} strokeWidth={EIGHTH_STROKE} strokeLinecap="round" />
          </Svg>
          {
            // Turned by the view around it, in React Native's own transform:
            // a CSS string on the SVG is the page's, and a device's SVG
            // refuses it.
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
