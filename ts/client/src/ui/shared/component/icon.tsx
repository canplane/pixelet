// @pxl/client/ui/shared/component/icon.tsx

import {
  ArchiveIcon,
  ArrowCircleUpIcon,
  ArrowUpIcon,
  BugIcon,
  CarIcon,
  ChatCircleIcon,
  CheckIcon,
  CubeIcon,
  EraserIcon,
  EyeIcon,
  GaugeIcon,
  GlobeIcon,
  HandPointingIcon,
  type Icon as PhosphorIcon,
  type IconWeight,
  ListIcon,
  MagnifyingGlassIcon,
  MapPinIcon,
  NavigationArrowIcon,
  PaletteIcon,
  PersonIcon,
  SailboatIcon,
  SpinnerIcon,
  SunIcon,
  TrainIcon,
  TreeIcon,
  XIcon,
} from "phosphor-react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Spin } from "@/ui/shared/component/motion/motion.tsx";

/**
 * A glyph is named by its SF Symbol and drawn off Apple's platforms as the
 * nearest Phosphor glyph, because Apple licenses its symbols to its own
 * platforms only. An entry holds the Phosphor glyph, whether it takes
 * Phosphor's fill weight (for a `.fill` name), and a turn in degrees
 * clockwise. The turn is there because Phosphor's navigation arrow points
 * up-left, where SF's points up-right.
 */
const PHOSPHOR = {
  archivebox: [ArchiveIcon, false, 0],
  "arrow.up": [ArrowUpIcon, false, 0],
  "bubble.left": [ChatCircleIcon, false, 0],
  car: [CarIcon, false, 0],
  checkmark: [CheckIcon, false, 0],
  "hand.point.up.left": [HandPointingIcon, false, 0],
  eraser: [EraserIcon, false, 0],
  eye: [EyeIcon, false, 0],
  "figure.stand": [PersonIcon, false, 0],
  "gauge.medium": [GaugeIcon, false, 0],
  globe: [GlobeIcon, false, 0],
  ladybug: [BugIcon, false, 0],
  "line.3.horizontal": [ListIcon, false, 0],
  location: [NavigationArrowIcon, false, 90],
  "location.fill": [NavigationArrowIcon, true, 90],
  "location.north.circle": [ArrowCircleUpIcon, false, 0],
  "location.north.line.fill": [NavigationArrowIcon, true, 45],
  magnifyingglass: [MagnifyingGlassIcon, false, 0],
  "mappin.circle": [MapPinIcon, false, 0],
  "mappin.circle.fill": [MapPinIcon, true, 0],
  "move.3d": [CubeIcon, false, 0],
  paintpalette: [PaletteIcon, false, 0],
  sailboat: [SailboatIcon, false, 0],
  "sun.max": [SunIcon, false, 0],
  "train.side.front.car": [TrainIcon, false, 0],
  tree: [TreeIcon, false, 0],
  xmark: [XIcon, false, 0],
} as const satisfies Record<string, readonly [PhosphorIcon, boolean, f64]>;

export type GlyphName = keyof typeof PHOSPHOR;

type GlyphWeight = "regular" | "semibold" | "bold" | "heavy" | "black";

/** Every SF weight above regular takes Phosphor's bold, the heaviest stroked
 * weight Phosphor has. */
const WEIGHT: Record<GlyphWeight, IconWeight> = {
  regular: "regular",
  semibold: "bold",
  bold: "bold",
  heavy: "bold",
  black: "bold",
};

export interface IconProps {
  glyph: GlyphName;
  size: f64;
  color: string;
  weight?: GlyphWeight;
}

export function Icon({ glyph, size, color, weight = "regular" }: IconProps) {
  const [Phosphor, filled, turn] = PHOSPHOR[glyph];
  return (
    <Phosphor
      size={size}
      color={color}
      weight={filled ? "fill" : WEIGHT[weight]}
      style={turn ? { transform: [{ rotate: `${turn}deg` }] } : undefined}
    />
  );
}

export function Spinner({ size, color }: { size: f64; color: string }) {
  return (
    <Spin periodMs={1000}>
      <SpinnerIcon size={size} color={color} weight="bold" />
    </Spin>
  );
}
