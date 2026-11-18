// @pxl/client/ui/shared/component/icon.tsx

import { View } from "react-native";
import Svg, { Circle, Path } from "react-native-svg";
import Box32 from "@carbon/icons/es/box/32.js";
import ArrowUp32 from "@carbon/icons/es/arrow--up/32.js";
import Car32 from "@carbon/icons/es/car/32.js";
import Chat32 from "@carbon/icons/es/chat/32.js";
import Checkbox32 from "@carbon/icons/es/checkbox/32.js";
import CheckboxChecked32 from "@carbon/icons/es/checkbox--checked/32.js";
import Checkmark32 from "@carbon/icons/es/checkmark/32.js";
import ChevronDown32 from "@carbon/icons/es/chevron--down/32.js";
import ChevronUp32 from "@carbon/icons/es/chevron--up/32.js";
import CircleDash32 from "@carbon/icons/es/circle-dash/32.js";
import Close32 from "@carbon/icons/es/close/32.js";
import Compass32 from "@carbon/icons/es/compass/32.js";
import Cube32 from "@carbon/icons/es/cube/32.js";
import CubeView32 from "@carbon/icons/es/cube-view/32.js";
import Cursor_132 from "@carbon/icons/es/cursor--1/32.js";
import Debug32 from "@carbon/icons/es/debug/32.js";
import Earth32 from "@carbon/icons/es/earth/32.js";
import Erase32 from "@carbon/icons/es/erase/32.js";
import Location32 from "@carbon/icons/es/location/32.js";
import LocationCurrent32 from "@carbon/icons/es/location--current/32.js";
import LocationFilled32 from "@carbon/icons/es/location--filled/32.js";
import Menu32 from "@carbon/icons/es/menu/32.js";
import Person32 from "@carbon/icons/es/person/32.js";
import SailboatCoastal32 from "@carbon/icons/es/sailboat--coastal/32.js";
import Search32 from "@carbon/icons/es/search/32.js";
import SendAlt32 from "@carbon/icons/es/send--alt/32.js";
import Sun32 from "@carbon/icons/es/sun/32.js";
import Train32 from "@carbon/icons/es/train/32.js";
import Sprout32 from "@carbon/icons/es/sprout/32.js";
import View32 from "@carbon/icons/es/view/32.js";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Spin } from "@/ui/shared/component/motion/motion.tsx";

/**
 * A glyph is named by its SF Symbol and drawn off Apple's platforms as the
 * nearest of IBM's Carbon icons, because Apple licenses its symbols to its own
 * platforms only. An entry holds the Carbon icon, at its 32 grid, and a turn
 * in degrees clockwise. Carbon draws one weight, so an SF weight is Apple's
 * alone, and a `.fill` name takes Carbon's filled icon where it has one.
 */
type CarbonIcon = typeof Box32;

const CARBON = {
  archivebox: [Box32, 0],
  "arrow.up": [ArrowUp32, 0],
  "bubble.left": [Chat32, 0],
  car: [Car32, 0],
  checkmark: [Checkmark32, 0],
  "checkmark.square": [CheckboxChecked32, 0],
  "chevron.down": [ChevronDown32, 0],
  "chevron.up": [ChevronUp32, 0],
  cursorarrow: [Cursor_132, 0],
  eraser: [Erase32, 0],
  eye: [View32, 0],
  "figure.stand": [Person32, 0],
  globe: [Earth32, 0],
  ladybug: [Debug32, 0],
  "line.3.horizontal": [Menu32, 0],
  location: [SendAlt32, 0],
  "location.fill": [LocationCurrent32, 0],
  "location.north.circle": [Compass32, 0],
  "location.north.line.fill": [LocationCurrent32, -45],
  magnifyingglass: [Search32, 0],
  "mappin.circle": [Location32, 0],
  "mappin.circle.fill": [LocationFilled32, 0],
  "move.3d": [Cube32, 0],
  sailboat: [SailboatCoastal32, 0],
  square: [Checkbox32, 0],
  "sun.max": [Sun32, 0],
  "train.side.front.car": [Train32, 0],
  tree: [Sprout32, 0],
  "view.3d": [CubeView32, 0],
  xmark: [Close32, 0],
} as const satisfies Record<string, readonly [CarbonIcon, f64]>;

export type GlyphName = keyof typeof CARBON;

type GlyphWeight = "regular" | "semibold" | "bold" | "heavy" | "black";

export interface IconProps {
  glyph: GlyphName;
  size: f64;
  color: string;
  weight?: GlyphWeight;
}

export function Icon({ glyph, size, color }: IconProps) {
  const [icon, turn] = CARBON[glyph];
  return <Carbon icon={icon} size={size} color={color} turnDeg={turn} />;
}

export function Spinner({ size, color }: { size: f64; color: string }) {
  return (
    <Spin periodMs={1000}>
      <Carbon icon={CircleDash32} size={size} color={color} />
    </Spin>
  );
}

/** A Carbon icon as its package lays it out: an SVG's view box and the paths
 * and circles in it, drawn in `color` and turned `turnDeg` degrees clockwise. */
function Carbon({
  icon,
  size,
  color,
  turnDeg = 0,
}: {
  icon: CarbonIcon;
  size: f64;
  color: string;
  turnDeg?: f64;
}) {
  return (
    <View
      style={turnDeg ? { transform: [{ rotate: `${turnDeg}deg` }] } : undefined}
    >
      <Svg width={size} height={size} viewBox={icon.attrs.viewBox} fill={color}>
        {icon.content.map((e, i) =>
          e.elem === "circle" ? (
            <Circle
              key={i}
              cx={e.attrs.cx}
              cy={e.attrs.cy}
              r={e.attrs.r}
              fill={e.attrs.fill}
            />
          ) : (
            <Path key={i} d={e.attrs.d} transform={e.attrs.transform} />
          ),
        )}
      </Svg>
    </View>
  );
}
