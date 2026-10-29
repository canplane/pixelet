// @pxl/client/shell/ui/icon.tsx

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
import { Spin } from "@/shell/ui/motion/motion.tsx";

/**
 * SF Symbol names, drawn as the nearest Phosphor glyph: Apple licenses the
 * symbols to its own platforms only. A `.fill` name is Phosphor's fill
 * weight; `turn` is degrees clockwise, Phosphor's navigation arrow pointing
 * up-left where SF's points up-right.
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

/** Phosphor has two stroked weights; SF's heavier ones take its bold. */
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
  const [Drawn, filled, turn] = PHOSPHOR[glyph];
  return (
    <Drawn
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
