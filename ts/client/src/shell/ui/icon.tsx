// @pxl/client/shell/ui/icon.tsx

import {
  Archive,
  ArrowCircleUp,
  ArrowUp,
  Bug,
  Car,
  ChatCircle,
  Check,
  Cube,
  Eraser,
  Eye,
  Gauge,
  Globe,
  HandPointing,
  type Icon as PhosphorIcon,
  type IconWeight,
  List,
  MagnifyingGlass,
  MapPin,
  NavigationArrow,
  Palette,
  Person,
  Sailboat,
  Spinner as SpinnerGlyph,
  Sun,
  Train,
  Tree,
  X,
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
  "archivebox": [Archive, false, 0],
  "arrow.up": [ArrowUp, false, 0],
  "bubble.left": [ChatCircle, false, 0],
  "car": [Car, false, 0],
  "checkmark": [Check, false, 0],
  "hand.point.up.left": [HandPointing, false, 0],
  "eraser": [Eraser, false, 0],
  "eye": [Eye, false, 0],
  "figure.stand": [Person, false, 0],
  "gauge.medium": [Gauge, false, 0],
  "globe": [Globe, false, 0],
  "ladybug": [Bug, false, 0],
  "line.3.horizontal": [List, false, 0],
  "location": [NavigationArrow, false, 90],
  "location.fill": [NavigationArrow, true, 90],
  "location.north.circle": [ArrowCircleUp, false, 0],
  "location.north.line.fill": [NavigationArrow, true, 45],
  "magnifyingglass": [MagnifyingGlass, false, 0],
  "mappin.circle": [MapPin, false, 0],
  "mappin.circle.fill": [MapPin, true, 0],
  "move.3d": [Cube, false, 0],
  "paintpalette": [Palette, false, 0],
  "sailboat": [Sailboat, false, 0],
  "sun.max": [Sun, false, 0],
  "train.side.front.car": [Train, false, 0],
  "tree": [Tree, false, 0],
  "xmark": [X, false, 0],
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
      <SpinnerGlyph size={size} color={color} weight="bold" />
    </Spin>
  );
}
